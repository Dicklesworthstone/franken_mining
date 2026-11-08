//! Untrusted split-destination PCPSP schedules (plan §15.4). No I/O or solver is
//! involved: fractions are exact decimal integers, and objectives and resource
//! uses are recomputed from the supplied model. No claimed objective is read.
//! Determinism: BitwisePortable
//!
//! A row is `<block> <destination> <period> <fraction>`. A mined block must have
//! exactly one mining period and fractions summing to exactly one, although its
//! material may go to several destinations. Zero rows do not mine a block.
//! Parsing checks structure; [`check`] reports incomplete blocks, unmet mining
//! precedence and exceeded capacities without a rounding tolerance. Thus a
//! published solution rounded slightly outside a limit remains infeasible.
//!
//! All checked totals are integer numerators over [`SplitCheck::scale`], in the
//! model's own integer value/resource coordinates. The caller retains the
//! model's decimal scales, source digests and discount convention. This checks
//! feasibility, not optimality, and supplies no dual bound.

use fmine_evidence::Refusal;

use super::PcpspModel;
use crate::pitflow::minelib::{MAX_DECIMALS, Rounding, bad, lines, number, scaled};
use crate::toposort::{WITNESSES, refuse};

/// One destination's fraction of a block, in the schedule's decimal scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestinationShare {
    pub block: u32,
    pub destination: u32,
    pub period: u32,
    /// An integer between zero and the schedule's scale, inclusive.
    pub fraction: i64,
}

/// Canonical positive allocations, ordered by block then destination. This is
/// structurally valid input, not a claim that the mining schedule is feasible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitSchedule {
    decimals: u32,
    assignments: Vec<DestinationShare>,
}

fn share_scale(decimals: u32) -> Result<i64, Refusal> {
    if decimals > MAX_DECIMALS {
        return Err(bad(format!(
            "PCPSP solution: {decimals} fraction decimals; at most {MAX_DECIMALS}"
        )));
    }
    Ok(10_i64.pow(decimals))
}

fn row_limit(model: &PcpspModel) -> Result<usize, Refusal> {
    model
        .blocks()
        .checked_mul(model.destinations())
        .ok_or_else(|| {
            bad("PCPSP solution: block/destination population exceeds addressable memory".into())
        })
}

fn admit_row(model: &PcpspModel, row: &DestinationShare, scale: i64) -> Result<(), Refusal> {
    if row.block as usize >= model.blocks()
        || row.destination as usize >= model.destinations()
        || row.period as usize >= model.periods()
        || !(0..=scale).contains(&row.fraction)
    {
        return Err(bad(format!(
            "PCPSP solution: block {}, destination {}, period {}, fraction {}/{} outside {} blocks, {} destinations, {} periods and [0,1]",
            row.block,
            row.destination,
            row.period,
            row.fraction,
            scale,
            model.blocks(),
            model.destinations(),
            model.periods()
        )));
    }
    Ok(())
}

impl SplitSchedule {
    /// Validate and canonicalize exact integer allocations. Duplicate triples
    /// are refused, including zero rows. A block's positive allocations must
    /// have one period; zero rows are then omitted. No fraction is rounded or
    /// normalized, and an incomplete block is retained for [`check`] to report.
    /// At most `blocks * destinations` rows may be supplied.
    pub fn new(
        model: &PcpspModel,
        mut assignments: Vec<DestinationShare>,
        share_decimals: u32,
    ) -> Result<Self, Refusal> {
        let scale = share_scale(share_decimals)?;
        if assignments.len() > row_limit(model)? {
            return Err(bad(
                "PCPSP solution: more rows than block/destination pairs".into(),
            ));
        }
        for row in &assignments {
            admit_row(model, row, scale)?;
        }
        assignments.sort_unstable_by_key(|a| (a.block, a.destination, a.period));
        if let Some(pair) = assignments.windows(2).find(|p| {
            (p[0].block, p[0].destination, p[0].period)
                == (p[1].block, p[1].destination, p[1].period)
        }) {
            return Err(bad(format!(
                "PCPSP solution: block {}, destination {}, period {} is listed twice",
                pair[0].block, pair[0].destination, pair[0].period
            )));
        }
        assignments.retain(|a| a.fraction != 0);
        for pair in assignments.windows(2) {
            if pair[0].block == pair[1].block && pair[0].period != pair[1].period {
                return Err(bad(format!(
                    "PCPSP solution: block {} is mined in more than one period",
                    pair[0].block
                )));
            }
        }
        Ok(Self {
            decimals: share_decimals,
            assignments,
        })
    }

    #[must_use]
    pub fn share_decimals(&self) -> u32 {
        self.decimals
    }

    #[must_use]
    pub fn scale(&self) -> i64 {
        10_i64.pow(self.decimals)
    }

    #[must_use]
    pub fn assignments(&self) -> &[DestinationShare] {
        &self.assignments
    }
}

/// Read MineLib's four-column PCPSP solution format; `%` comments and blanks are
/// ignored. All decimal fractions must be exactly representable at the supplied
/// scale (at most nine places). Extra fields, invalid indices, repeated rows,
/// negative or above-one fractions and multi-period blocks refuse. An invalid
/// total fraction remains a candidate that [`check`] diagnoses as infeasible.
pub fn parse_solution(
    text: &str,
    model: &PcpspModel,
    share_decimals: u32,
) -> Result<SplitSchedule, Refusal> {
    let scale = share_scale(share_decimals)?;
    let limit = row_limit(model)?;
    let mut assignments = Vec::new();
    for (line, content) in lines(text) {
        if assignments.len() == limit {
            return Err(bad(format!(
                "line {line}: more solution rows than block/destination pairs"
            )));
        }
        let mut fields = content.split_whitespace();
        let (Some(b), Some(d), Some(t), Some(f), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            return Err(bad(format!(
                "line {line}: a solution line is <block> <destination> <period> <fraction>"
            )));
        };
        let index = |token, name| -> Result<u32, Refusal> {
            u32::try_from(number(token, name, line)?)
                .map_err(|_| bad(format!("line {line}: {name} {token} does not fit u32")))
        };
        let fraction = scaled(f, share_decimals, Rounding::Refuse)
            .map_err(|e| bad(format!("line {line}: {}", e.what)))?
            .0;
        let row = DestinationShare {
            block: index(b, "block")?,
            destination: index(d, "destination")?,
            period: index(t, "period")?,
            fraction,
        };
        admit_row(model, &row, scale).map_err(|e| bad(format!("line {line}: {}", e.what)))?;
        assignments.push(row);
    }
    SplitSchedule::new(model, assignments, share_decimals)
}

/// Lift a whole-block schedule into the same exact candidate checker and
/// writer. Both vectors must name every block, including the unmined ones.
pub fn from_whole(
    model: &PcpspModel,
    schedule: &[Option<u32>],
    destinations: &[u32],
    share_decimals: u32,
) -> Result<SplitSchedule, Refusal> {
    let scale = share_scale(share_decimals)?;
    if schedule.len() != model.blocks() || destinations.len() != model.blocks() {
        return Err(bad(format!(
            "PCPSP solution: {} periods and {} destinations for {} blocks",
            schedule.len(),
            destinations.len(),
            model.blocks()
        )));
    }
    let mut assignments = Vec::new();
    for (b, (&period, &destination)) in schedule.iter().zip(destinations).enumerate() {
        if destination as usize >= model.destinations() {
            return Err(bad(format!(
                "PCPSP solution: block {b} destination {destination} is outside the model"
            )));
        }
        if let Some(period) = period {
            assignments.push(DestinationShare {
                block: u32::try_from(b).map_err(|_| bad(format!("block {b} does not fit u32")))?,
                destination,
                period,
                fraction: scale,
            });
        }
    }
    SplitSchedule::new(model, assignments, share_decimals)
}

/// Canonical exact MineLib rows, with no header, objective, zero rows or trailing
/// decimal zeros. Re-reading at the same scale preserves the candidate exactly.
#[must_use]
pub fn write_solution(schedule: &SplitSchedule) -> String {
    use std::fmt::Write as _;
    let scale = schedule.scale();
    let mut out = String::new();
    for row in schedule.assignments() {
        let fraction = if row.fraction == scale {
            "1".to_string()
        } else {
            format!(
                "0.{:0width$}",
                row.fraction,
                width = schedule.decimals as usize
            )
            .trim_end_matches('0')
            .to_string()
        };
        let _ = writeln!(
            out,
            "{} {} {} {fraction}",
            row.block, row.destination, row.period
        );
    }
    out
}

/// Exact feasibility and independently recomputed totals. Resource and value
/// numerators share `scale`; resource capacities remain in the model's units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitCheck {
    pub scale: i64,
    /// Blocks with any positive destination fraction, including incomplete ones.
    pub mined: usize,
    pub period_values: Vec<i128>,
    /// `used[resource][period]`, including signed blend-resource consumption.
    pub used: Vec<Vec<i128>>,
    /// Mined blocks whose fractions do not sum to exactly `scale`.
    pub incomplete: usize,
    /// First [`WITNESSES`] `(block, total_fraction)` pairs, in block order.
    pub first_incomplete: Vec<(u32, i128)>,
    /// Unmet predecessor arcs; a predecessor must be completely mined by then.
    pub unmet: usize,
    pub first_unmet: Vec<(u32, u32)>,
    /// Every violated capacity, in resource then period order.
    pub over_limit: Vec<(usize, usize)>,
}

fn arithmetic(detail: &str) -> Refusal {
    refuse(format!(
        "PCPSP candidate: exact integer arithmetic overflow ({detail})"
    ))
}

fn add_product(sum: &mut i128, value: i64, fraction: i64) -> Result<(), Refusal> {
    let term = i128::from(value)
        .checked_mul(i128::from(fraction))
        .ok_or_else(|| arithmetic("coefficient times fraction"))?;
    *sum = sum
        .checked_add(term)
        .ok_or_else(|| arithmetic("accumulated total"))?;
    Ok(())
}

impl SplitCheck {
    #[must_use]
    pub fn feasible(&self) -> bool {
        self.incomplete == 0 && self.unmet == 0 && self.over_limit.is_empty()
    }

    /// Exact undiscounted objective numerator over `scale`, even for an
    /// infeasible candidate. This never uses a solver's claimed objective.
    pub fn value(&self) -> Result<i128, Refusal> {
        self.period_values.iter().try_fold(0_i128, |sum, &value| {
            sum.checked_add(value)
                .ok_or_else(|| arithmetic("objective"))
        })
    }

    /// Exact `sum(period_values[t] * factors[t])`, requiring one nonnegative
    /// integer factor per period. The result is a numerator over `scale` times
    /// the caller's factor scale (normally `search::DISCOUNT_SCALE`). It remains
    /// in the model's integer value units. Overflow refuses; no ratio is rounded.
    pub fn discounted_value(&self, factors: &[i64]) -> Result<i128, Refusal> {
        if factors.len() != self.period_values.len() || factors.iter().any(|&f| f < 0) {
            return Err(refuse(
                "PCPSP candidate: one nonnegative discount factor per period is required".into(),
            ));
        }
        self.period_values
            .iter()
            .zip(factors)
            .try_fold(0_i128, |sum, (&value, &factor)| {
                let term = value
                    .checked_mul(i128::from(factor))
                    .ok_or_else(|| arithmetic("discounted period value"))?;
                sum.checked_add(term)
                    .ok_or_else(|| arithmetic("discounted objective"))
            })
    }
}

/// Check the candidate against this model without calling any scheduling or LP
/// algorithm. Fractions are never normalized, negative resource uses are never
/// dropped, and partial predecessor extraction does not satisfy precedence.
/// A structurally valid candidate may fail this check; that is a diagnostic
/// result. Incompatible model dimensions or integer overflow refuse instead.
pub fn check(model: &PcpspModel, schedule: &SplitSchedule) -> Result<SplitCheck, Refusal> {
    let scale = schedule.scale();
    let mut periods = vec![None; model.blocks()];
    let mut fractions = vec![0_i128; model.blocks()];
    let mut out = SplitCheck {
        scale,
        mined: 0,
        period_values: vec![0; model.periods()],
        used: vec![vec![0; model.periods()]; model.resources()],
        incomplete: 0,
        first_incomplete: Vec::new(),
        unmet: 0,
        first_unmet: Vec::new(),
        over_limit: Vec::new(),
    };
    for a in schedule.assignments() {
        admit_row(model, a, scale)?;
        let (b, d, t) = (a.block as usize, a.destination as usize, a.period as usize);
        periods[b] = Some(a.period);
        fractions[b] = fractions[b]
            .checked_add(i128::from(a.fraction))
            .ok_or_else(|| arithmetic("block fractions"))?;
        let slot = b * model.destinations() + d;
        add_product(&mut out.period_values[t], model.values[slot], a.fraction)?;
        for (used, uses) in out.used.iter_mut().zip(&model.uses) {
            add_product(&mut used[t], uses[slot], a.fraction)?;
        }
    }
    for (b, (&period, &fraction)) in periods.iter().zip(&fractions).enumerate() {
        let Some(t) = period else { continue };
        out.mined += 1;
        let block = u32::try_from(b).map_err(|_| refuse("PCPSP block index exceeds u32".into()))?;
        if fraction != i128::from(scale) {
            out.incomplete += 1;
            if out.first_incomplete.len() < WITNESSES {
                out.first_incomplete.push((block, fraction));
            }
        }
        for &required in &model.requires[b] {
            let q = required as usize;
            if fractions[q] != i128::from(scale) || periods[q].is_none_or(|qt| qt > t) {
                out.unmet += 1;
                if out.first_unmet.len() < WITNESSES {
                    out.first_unmet.push((block, required));
                }
            }
        }
    }
    for (r, (used, limits)) in out.used.iter().zip(&model.limits).enumerate() {
        for (t, (&usage, &limit)) in used.iter().zip(limits).enumerate() {
            let cap = i128::from(limit)
                .checked_mul(i128::from(scale))
                .ok_or_else(|| arithmetic("scaled resource limit"))?;
            if usage > cap {
                out.over_limit.push((r, t));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> PcpspModel {
        // Block 2 requires block 0. A mill fraction of block 0 consumes +2
        // blend units, whereas block 1 consumes -1. Each destination mines 1.
        super::super::parse(
            "NAME: split\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 2\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\n1 0 L 0\n1 1 L 0\nOBJECTIVE_FUNCTION:\n0 -4 12\n1 -2 8\n2 -3 21\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 0 0 1\n0 1 0 1\n0 1 1 2\n1 0 0 1\n1 1 0 1\n1 1 1 -1\n2 0 0 1\n2 1 0 1\nEOF\n",
            "0 0\n1 0\n2 1 0\n", 0, Rounding::Refuse, 0,
        ).unwrap()
    }

    const SOLUTION: &str = "0 0 0 0.5\n0 1 0 0.5\n1 1 0 1\n2 1 1 1\n";

    #[test]
    fn split_known_answer_recomputes_signed_resources_value_and_discount() {
        let m = model();
        let s = parse_solution(SOLUTION, &m, 3).unwrap();
        let c = check(&m, &s).unwrap();
        assert!(c.feasible());
        assert_eq!(c.mined, 3);
        // Period 0: -4/2 + 12/2 + 8 = 12; period 1: 21.
        assert_eq!(c.period_values, [12_000, 21_000]);
        assert_eq!(c.used, [vec![2_000, 1_000], vec![0, 0]]);
        assert_eq!(c.value().unwrap(), 33_000);
        // With factors 1 and 3/4, the discounted value is 111/4.
        assert_eq!(c.discounted_value(&[4, 3]).unwrap(), 111_000);
        assert_eq!(write_solution(&s), SOLUTION);
        assert_eq!(parse_solution(&write_solution(&s), &m, 3).unwrap(), s);
    }

    #[test]
    fn canonical_order_exact_exponents_and_zero_rows() {
        let m = model();
        let s = parse_solution(
            "% claimed objective: 999999, deliberately ignored\n\n2 1 1 1.000000000\n0 1 0 5e-1\n1 0 1 0\n1 1 0 1\n0 0 0 0.500000\n",
            &m, 9,
        ).unwrap();
        assert_eq!(write_solution(&s), SOLUTION);
        assert_eq!(s.assignments().len(), 4);
        assert_eq!(s.share_decimals(), 9);
        assert_eq!(s.scale(), 1_000_000_000);
        let empty = parse_solution("0 0 0 0\n", &m, 0).unwrap();
        assert!(empty.assignments().is_empty());
        assert!(check(&m, &empty).unwrap().feasible());
        assert_eq!(write_solution(&empty), "");
    }

    #[test]
    fn fractional_rounding_never_repairs_capacity_or_wholeness() {
        let m = model();
        let over = SOLUTION
            .replace("0 0 0 0.5", "0 0 0 0.499999999")
            .replace("0 1 0 0.5", "0 1 0 0.500000001");
        let c = check(&m, &parse_solution(&over, &m, 9).unwrap()).unwrap();
        assert!(!c.feasible());
        assert_eq!(c.incomplete, 0);
        assert_eq!(c.over_limit, [(1, 0)]);
        assert_eq!(c.used[1][0], 2);
        let short = SOLUTION.replace("0 0 0 0.5", "0 0 0 0.499999999");
        let c = check(&m, &parse_solution(&short, &m, 9).unwrap()).unwrap();
        assert!(!c.feasible());
        assert_eq!(c.first_incomplete, [(0, 999_999_999)]);
        assert_eq!(c.first_unmet, [(2, 0)]);
        assert!(c.over_limit.is_empty());
        let excess = SOLUTION.replace("0 0 0 0.5", "0 0 0 0.500000001");
        let c = check(&m, &parse_solution(&excess, &m, 9).unwrap()).unwrap();
        assert_eq!(c.first_incomplete, [(0, 1_000_000_001)]);
        assert_eq!(c.over_limit, [(0, 0)]);
    }

    #[test]
    fn precedence_accepts_same_period_and_refuses_late_or_missing_predecessors() {
        let m = model();
        let same = parse_solution("0 0 1 1\n2 1 1 1\n", &m, 0).unwrap();
        assert!(check(&m, &same).unwrap().feasible());
        let late = parse_solution("0 0 1 1\n2 1 0 1\n", &m, 0).unwrap();
        assert_eq!(check(&m, &late).unwrap().first_unmet, [(2, 0)]);
        let absent = parse_solution("2 1 0 1\n", &m, 0).unwrap();
        assert_eq!(check(&m, &absent).unwrap().first_unmet, [(2, 0)]);
    }

    #[test]
    fn malformed_rows_and_unrepresentable_fractions_refuse() {
        let m = model();
        for text in [
            "0 0 0",
            "0 0 0 1 extra",
            "3 0 0 1",
            "0 2 0 1",
            "0 0 2 1",
            "-1 0 0 1",
            "0 0 0 -0.1",
            "0 0 0 1.000000001",
            "0 0 0 NaN",
            "0 0 0 0.0000000001",
            "0 0 0 0.5\n0 0 0 0.5",
            "0 0 0 0\n0 0 0 1",
            "0 0 0 0.5\n0 1 1 0.5",
            "0 0 0 0.5\n0 0 1 0.5",
            "0 0 0 1/2",
        ] {
            assert!(parse_solution(text, &m, 9).is_err(), "accepted {text:?}");
        }
        assert!(parse_solution("", &m, 10).is_err());
        assert!(
            parse_solution(&"0 0 0 0\n".repeat(7), &m, 9)
                .unwrap_err()
                .what
                .contains("more solution rows")
        );
        assert!(parse_solution("0 0 0 0.125", &m, 2).is_err());
        assert!(parse_solution("0 0 0 0.1250", &m, 3).is_ok());
    }

    #[test]
    fn whole_schedules_match_the_existing_independent_integer_checker() {
        let m = model();
        let destination = [0, 1, 1];
        let problem = m.with_destinations(&destination).unwrap();
        // All 3^3 unmined/period-0/period-1 schedules, including infeasible ones.
        for code in 0..27 {
            let mut n = code;
            let schedule: Vec<_> = (0..3)
                .map(|_| {
                    let q = n % 3;
                    n /= 3;
                    (q != 0).then(|| q - 1)
                })
                .collect();
            let old = crate::toposort::check(&problem, &schedule).unwrap();
            let s = from_whole(&m, &schedule, &destination, 3).unwrap();
            let new = check(&m, &s).unwrap();
            assert_eq!(new.feasible(), old.feasible());
            assert_eq!(new.mined, old.mined);
            assert_eq!(new.unmet, old.unmet);
            assert_eq!(new.first_unmet, old.first_unmet);
            assert_eq!(new.over_limit, old.over_limit);
            assert_eq!(
                new.period_values,
                old.period_values
                    .iter()
                    .map(|x| x * 1_000)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                new.used,
                old.used
                    .iter()
                    .map(|r| r.iter().map(|x| x * 1_000).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
        }
        assert!(from_whole(&m, &[None], &[0], 9).is_err());
        assert!(from_whole(&m, &[None; 3], &[0, 0, 2], 9).is_err());
        assert!(from_whole(&m, &[Some(2), None, None], &[0; 3], 9).is_err());
    }

    #[test]
    fn discounted_arithmetic_and_rechecking_against_changed_models_refuse_explicitly() {
        let m = model();
        let s = parse_solution(SOLUTION, &m, 9).unwrap();
        let mut c = check(&m, &s).unwrap();
        assert!(c.discounted_value(&[1]).is_err());
        assert!(c.discounted_value(&[1, -1]).is_err());
        c.period_values = vec![i128::MAX, 1];
        assert!(c.value().unwrap_err().what.contains("overflow"));
        assert!(
            c.discounted_value(&[1, 1])
                .unwrap_err()
                .what
                .contains("overflow")
        );
        c.period_values = vec![i128::MAX, 0];
        assert!(
            c.discounted_value(&[2, 1])
                .unwrap_err()
                .what
                .contains("overflow")
        );
        let mut expensive = m.clone();
        // Still below the model's 2^62 value bound. Fraction scaling followed
        // by the normal 2^40 discount scale no longer fits an i128 numerator.
        expensive.values[0] = 1_000_000_000_000_000_000;
        let whole = parse_solution("0 0 0 1\n", &expensive, 9).unwrap();
        let checked = check(&expensive, &whole).unwrap();
        assert!(checked.feasible());
        assert!(
            checked
                .discounted_value(&[crate::toposort::search::DISCOUNT_SCALE; 2])
                .unwrap_err()
                .what
                .contains("overflow")
        );
        let mut changed = m;
        changed.periods = 1;
        assert!(check(&changed, &s).is_err());
    }

    #[test]
    fn diagnostic_counts_are_complete_and_witnesses_canonical_and_bounded() {
        let mut m = model();
        m.values = vec![1; 30];
        m.uses = vec![];
        m.limits = vec![];
        m.requires = (0..15)
            .map(|b| if b == 0 { vec![] } else { vec![0] })
            .collect();
        let text = (1..15)
            .rev()
            .map(|b| format!("{b} 1 0 0.5\n"))
            .collect::<String>();
        let c = check(&m, &parse_solution(&text, &m, 1).unwrap()).unwrap();
        assert_eq!((c.mined, c.incomplete, c.unmet), (14, 14, 14));
        assert_eq!(
            c.first_incomplete,
            (1..=10).map(|b| (b, 5)).collect::<Vec<_>>()
        );
        assert_eq!(c.first_unmet, (1..=10).map(|b| (b, 0)).collect::<Vec<_>>());
        assert!(!c.feasible());
    }
}
