//! An external pit checked as an untrusted candidate (bd-nq06, plan §15.4 `UntrustedCandidate`): its closure and its
//! exact value, recomputed here. Nothing the candidate claims about itself is read.
//! Determinism: BitwisePortable (`i64` and `i128` integer arithmetic only; no floating point, hashing or threads).
//!
//! A candidate file lists the blocks in the pit as whitespace-separated block ids; lines starting with `%` are
//! comments. Every id must be a block of the problem, given once.

use fmine_evidence::{Refusal, RefusalCode};

use super::ClosureProblem;

/// How many unmet requirements are kept as witnesses; all of them are counted.
pub const WITNESSES: usize = 10;

/// What a candidate pit is, against the problem it was checked on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCheck {
    /// Blocks in the candidate.
    pub blocks: usize,
    /// The exact sum of the candidate's block values.
    pub value: i64,
    /// Requirements `(block, required)` with the block in the candidate and the required block out of it, counted
    /// per arc.
    pub unmet: usize,
    /// The first [`WITNESSES`] of them, by block then required block.
    pub first_unmet: Vec<(u32, u32)>,
}

impl CandidateCheck {
    /// Whether the candidate meets every requirement of its blocks.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.unmet == 0
    }
}

/// The candidate's block ids as membership over `n` blocks.
///
/// # Errors
/// `FMINE-REF-506` (proposed) for a token that is not a non-negative integer, an id that is not below `n`, or an id
/// given twice.
pub fn parse_ids(text: &str, n: usize) -> Result<Vec<bool>, Refusal> {
    let bad = |detail: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("candidate pit: {detail}"),
        )
    };
    let mut in_set = vec![false; n];
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('%') {
            continue;
        }
        for token in line.split_whitespace() {
            let id = token
                .parse::<usize>()
                .ok()
                .filter(|_| token.bytes().all(|b| b.is_ascii_digit()))
                .ok_or_else(|| bad(format!("line {}: {token:?} is not a block id", i + 1)))?;
            let slot = in_set.get_mut(id).ok_or_else(|| {
                bad(format!(
                    "line {}: block {id} is not one of {n} blocks",
                    i + 1
                ))
            })?;
            if std::mem::replace(slot, true) {
                return Err(bad(format!("line {}: block {id} is listed twice", i + 1)));
            }
        }
    }
    Ok(in_set)
}

/// Closure and exact value of `in_set` on `problem`.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `in_set` does not have one entry per block, or the value does not fit `i64`
/// (the problem's 2⁶² guard rules that out).
pub fn check(problem: &ClosureProblem, in_set: &[bool]) -> Result<CandidateCheck, Refusal> {
    let refuse = |what: String| Refusal::new(RefusalCode::NumericalCertificateFailed207, what);
    if in_set.len() != problem.node_count() {
        return Err(refuse(format!(
            "candidate pit: {} membership entries for {} blocks",
            in_set.len(),
            problem.node_count()
        )));
    }
    let (mut blocks, mut sum, mut unmet, mut first_unmet) = (0, 0_i128, 0, Vec::new());
    for (b, (&inside, &w)) in in_set.iter().zip(problem.weights()).enumerate() {
        if !inside {
            continue;
        }
        blocks += 1;
        sum += i128::from(w);
        for &p in problem.requires(b).unwrap_or(&[]) {
            if !in_set.get(p as usize).copied().unwrap_or(false) {
                unmet += 1;
                if first_unmet.len() < WITNESSES {
                    let block = u32::try_from(b)
                        .map_err(|_| refuse(format!("block {b} does not fit u32")))?;
                    first_unmet.push((block, p));
                }
            }
        }
    }
    let value = i64::try_from(sum)
        .map_err(|_| refuse("candidate pit: value does not fit i64".to_string()))?;
    Ok(CandidateCheck {
        blocks,
        value,
        unmet,
        first_unmet,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Block 3 requires 0, 1 and 2; block 4 requires 3.
    fn problem() -> ClosureProblem {
        ClosureProblem::from_requirements(
            vec![-150, -200, -100, 625, -10],
            &[vec![], vec![], vec![], vec![0, 1, 2], vec![3]],
        )
        .unwrap()
    }

    #[test]
    fn ids_parse_to_membership_and_bad_ids_refuse() {
        assert_eq!(
            parse_ids("% a comment 7 7\n3 0\n\n 1 2\n", 5).unwrap(),
            vec![true, true, true, true, false]
        );
        assert_eq!(parse_ids("", 2).unwrap(), vec![false, false]);
        for (text, detail) in [
            ("0 x", "\"x\" is not a block id"),
            ("+1", "\"+1\" is not a block id"),
            ("-1", "\"-1\" is not a block id"),
            ("0 5", "block 5 is not one of 5 blocks"),
            ("1\n3 1", "line 2: block 1 is listed twice"),
        ] {
            let refusal = parse_ids(text, 5).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-506");
            assert!(refusal.what.contains(detail), "{text:?}: {}", refusal.what);
        }
    }

    #[test]
    fn closure_witnesses_and_value_are_exact() {
        let p = problem();
        let optimum = check(&p, &[true, true, true, true, false]).unwrap();
        assert_eq!((optimum.blocks, optimum.value, optimum.unmet), (4, 175, 0));
        assert!(optimum.closed() && optimum.first_unmet.is_empty());
        let open = check(&p, &[false, true, false, true, true]).unwrap();
        assert_eq!((open.blocks, open.value, open.unmet), (3, 415, 2));
        assert_eq!(open.first_unmet, vec![(3, 0), (3, 2)]);
        assert!(!open.closed());
        assert_eq!(check(&p, &[false; 5]).unwrap().value, 0);
        assert_eq!(check(&p, &[true; 4]).unwrap_err().code, "FMINE-REF-207");
    }

    #[test]
    fn witnesses_stop_at_the_limit_but_the_count_does_not() {
        let n = WITNESSES + 5;
        let requires: Vec<Vec<u32>> = (0..n)
            .map(|b| if b == 0 { vec![] } else { vec![0] })
            .collect();
        let p = ClosureProblem::from_requirements(vec![1; n], &requires).unwrap();
        let mut in_set = vec![true; n];
        in_set[0] = false;
        let c = check(&p, &in_set).unwrap();
        assert_eq!((c.unmet, c.first_unmet.len()), (n - 1, WITNESSES));
        assert_eq!(c.first_unmet[0], (1, 0));
    }
}
