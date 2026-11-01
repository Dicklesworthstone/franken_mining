//! DESIGN (BD-345.S4): derive one additive quarter from cumulative displayed quantities.
//! Determinism: Exact
//! Every admitted candidate pair remains visible; agreement with a draft cannot select
//! a preferred vintage. Closed rounding boxes are conservative enclosures, never confidence intervals.

use std::collections::BTreeSet;

use fmine_numerics::exact_feas::{Convention, Interval, Rat};

use super::js;

pub const WARNING: &str = "Arithmetic over public assertions does not make a derived figure an issuer disclosure. A non-obvious derived figure can itself be new information; obtain appropriate disclosure review.";

/// Only additive quantities are representable. A cost rate, grade or recovery cannot enter this API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantity {
    GoldProductionKoz,
    GoldSalesKoz,
    CopperProductionKt,
}

impl Quantity {
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::GoldProductionKoz | Self::GoldSalesKoz => "koz",
            Self::CopperProductionKt => "kt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Printed {
    pub value: Rat,
    pub step: Rat,
}

impl Printed {
    /// Unknown or nonpositive precision refuses. The closed half-step envelope assumes no tie convention.
    pub fn interval(self) -> Result<Interval, String> {
        if self.step.signum() <= 0 {
            return Err("displayed rounding step must be positive".into());
        }
        let half = self
            .step
            .checked_div(Rat::from_integer(2))
            .map_err(|e| e.to_string())?;
        Convention::Closed
            .interval(self.value, half)
            .map_err(|_| "displayed interval overflow".into())
    }
}

/// The caller authenticates the evidence and publication cut. A series key binds issuer, subject, year,
/// scope/perimeter version, metric, unit, occurrence and an explicit normalized ownership basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub source_id: String,
    pub series: String,
    /// January through the end of quarter 1, 2, 3 or 4. A standalone Q2 or Q3 is not cumulative.
    pub through_quarter: u8,
    /// Missing, status-marked or uninterpretable printed values stay unavailable, never zero.
    pub printed: Option<Printed>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Derived,
    PrecisionNotEstablished,
    Contradicted,
    ConflictingDerivations,
    NotAvailable,
    Refused,
}

impl Status {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
            Self::PrecisionNotEstablished => "precision_not_established",
            Self::Contradicted => "contradicted",
            Self::ConflictingDerivations => "conflicting_derivations",
            Self::NotAvailable => "not_available",
            Self::Refused => "refused",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub cumulative_claim: String,
    pub prior_claim: String,
    pub interval: Interval,
    pub relation: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub quantity: Quantity,
    pub quarter: u8,
    pub status: Status,
    pub reason: String,
    pub draft_interval: Option<Interval>,
    pub pairs: Vec<Pair>,
}

fn relation(draft: Interval, source: Interval) -> Result<Status, String> {
    let cmp = |a: Rat, b: Rat| a.checked_cmp(b).map_err(|e| e.to_string());
    Ok(
        if cmp(draft.lo, source.lo)?.is_le() && cmp(draft.hi, source.hi)?.is_ge() {
            Status::Derived
        } else if cmp(draft.hi, source.lo)?.is_lt() || cmp(draft.lo, source.hi)?.is_gt() {
            Status::Contradicted
        } else {
            Status::PrecisionNotEstablished
        },
    )
}

fn common_box(values: &[(&Observation, Interval)]) -> Result<bool, String> {
    let Some((_, first)) = values.first() else {
        return Ok(false);
    };
    let (mut lo, mut hi) = (first.lo, first.hi);
    for (_, value) in values.iter().skip(1) {
        if value.lo.checked_cmp(lo).map_err(|e| e.to_string())?.is_gt() {
            lo = value.lo;
        }
        if value.hi.checked_cmp(hi).map_err(|e| e.to_string())?.is_lt() {
            hi = value.hi;
        }
    }
    Ok(lo.checked_cmp(hi).map_err(|e| e.to_string())?.is_le())
}

/// Assess every compatible pre-cut cumulative pair. No point estimate, source ranking or silent exclusion
/// is introduced. A negative difference retains its sign; future publication admission is a caller duty.
#[must_use]
pub fn assess(
    quantity: Quantity,
    quarter: u8,
    draft: Printed,
    observations: &[Observation],
) -> Report {
    let mut report = Report {
        quantity,
        quarter,
        status: Status::Refused,
        reason: String::new(),
        draft_interval: None,
        pairs: Vec::new(),
    };
    let run = (|| -> Result<Status, String> {
        if !(2..=4).contains(&quarter) {
            return Err("only Q2, Q3 and Q4 have a registered cumulative-difference rule".into());
        }
        let draft = draft.interval()?;
        report.draft_interval = Some(draft);
        let mut ids = BTreeSet::new();
        let mut series = BTreeSet::new();
        let (mut end, mut prior) = (Vec::new(), Vec::new());
        for observation in observations {
            if observation.source_id.is_empty()
                || observation.series.is_empty()
                || !ids.insert(&observation.source_id)
            {
                return Err("empty or duplicate source identity, or empty series key".into());
            }
            if observation.through_quarter != quarter && observation.through_quarter != quarter - 1
            {
                return Err("observation is not a required cumulative period".into());
            }
            series.insert(&observation.series);
            let value = observation
                .printed
                .ok_or_else(|| {
                    format!(
                        "{}: admitted cumulative evidence has no usable displayed number",
                        observation.source_id
                    )
                })?
                .interval()?;
            if observation.through_quarter == quarter {
                end.push((observation, value));
            } else {
                prior.push((observation, value));
            }
        }
        if series.len() > 1 {
            return Err("cumulative candidates do not have one unambiguous semantic, ownership and perimeter basis".into());
        }
        if end.is_empty() || prior.is_empty() {
            return Ok(Status::NotAvailable);
        }
        if end.len().checked_mul(prior.len()).is_none_or(|n| n > 4096) {
            return Err("cumulative comparison exceeds the complete-pair limit".into());
        }
        end.sort_by_key(|(s, _)| &s.source_id);
        prior.sort_by_key(|(s, _)| &s.source_id);
        for (cumulative, a) in &end {
            for (previous, b) in &prior {
                let interval = Interval {
                    lo: a.lo.checked_sub(b.hi).map_err(|e| e.to_string())?,
                    hi: a.hi.checked_sub(b.lo).map_err(|e| e.to_string())?,
                    lo_strict: false,
                    hi_strict: false,
                };
                report.pairs.push(Pair {
                    cumulative_claim: cumulative.source_id.clone(),
                    prior_claim: previous.source_id.clone(),
                    interval,
                    relation: relation(draft, interval)?,
                });
            }
        }
        if !common_box(&end)? || !common_box(&prior)? {
            return Ok(Status::ConflictingDerivations);
        }
        let has = |status| report.pairs.iter().any(|p| p.relation == status);
        Ok(
            if report.pairs.iter().all(|p| p.relation == Status::Derived) {
                Status::Derived
            } else if has(Status::Contradicted) {
                if report
                    .pairs
                    .iter()
                    .all(|p| p.relation == Status::Contradicted)
                {
                    Status::Contradicted
                } else {
                    Status::ConflictingDerivations
                }
            } else {
                Status::PrecisionNotEstablished
            },
        )
    })();
    match run {
        Ok(status) => {
            report.status = status;
            report.reason = match status {
                Status::Derived => "every admitted cumulative pair's exact enclosure fits the draft precision",
                Status::NotAvailable => "at least one required cumulative period has no admitted source",
                Status::PrecisionNotEstablished => "the cumulative rounding uncertainty exceeds the draft precision",
                Status::Contradicted => "every admitted cumulative difference is disjoint from the draft interval",
                Status::ConflictingDerivations => "competing admitted cumulative evidence cannot establish one consistent derivation",
                Status::Refused => "derivation refused",
            }.into();
        }
        Err(reason) => report.reason = reason,
    }
    report
}

fn interval_json(i: Interval) -> String {
    let exact = |r: Rat| {
        if r.den() == 1 {
            r.num().to_string()
        } else {
            format!("{}/{}", r.num(), r.den())
        }
    };
    format!(
        "{{\"lower\":{},\"upper\":{}}}",
        js(&exact(i.lo)),
        js(&exact(i.hi))
    )
}

impl Report {
    #[must_use]
    pub const fn rule(&self) -> &'static str {
        match self.quarter {
            2 => "H1-Q1",
            3 => "9M-H1",
            4 => "FY-9M",
            _ => "unsupported",
        }
    }

    /// Stable fields for an evidence adapter to enrich with authenticated source metadata.
    #[must_use]
    pub fn body_json(&self) -> String {
        let pairs = self.pairs.iter().map(|p| format!("{{\"cumulative_claim\":{},\"prior_claim\":{},\"interval\":{},\"relation\":{}}}", js(&p.cumulative_claim), js(&p.prior_claim), interval_json(p.interval), js(p.relation.as_str()))).collect::<Vec<_>>().join(",");
        format!(
            "\"status\":{},\"reason\":{},\"rule\":{},\"unit\":{},\"draft_interval\":{},\"pairs\":[{}],\"warning\":{}",
            js(self.status.as_str()),
            js(&self.reason),
            js(self.rule()),
            js(self.quantity.unit()),
            self.draft_interval
                .map_or_else(|| "null".into(), interval_json),
            pairs,
            js(WARNING),
        )
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!("{{{}}}", self.body_json())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printed(value: &str, step: &str) -> Printed {
        Printed {
            value: Rat::from_decimal_str(value).unwrap(),
            step: Rat::from_decimal_str(step).unwrap(),
        }
    }
    fn observation(id: &str, quarter: u8, value: &str) -> Observation {
        Observation {
            source_id: id.into(),
            series: "invented-same-basis".into(),
            through_quarter: quarter,
            printed: Some(printed(value, "0.1")),
        }
    }
    fn run(quarter: u8, draft: Printed, inputs: &[Observation]) -> Report {
        assess(Quantity::GoldProductionKoz, quarter, draft, inputs)
    }

    #[test]
    fn cumulative_rules_preserve_exact_rounding_and_signed_zero_results() {
        for quarter in 2..=4 {
            for (end, prior, result) in [
                ("200", "100", "100"),
                ("100", "100", "0"),
                ("50", "100", "-50"),
            ] {
                let inputs = [
                    observation("end", quarter, end),
                    observation("prior", quarter - 1, prior),
                ];
                let report = run(quarter, printed(result, "1"), &inputs);
                assert_eq!(report.status, Status::Derived);
                let expected = Rat::from_decimal_str(result).unwrap();
                assert_eq!(
                    report.pairs[0].interval.lo,
                    expected.checked_sub(Rat::new(1, 10).unwrap()).unwrap()
                );
                assert_eq!(
                    report.pairs[0].interval.hi,
                    expected.checked_add(Rat::new(1, 10).unwrap()).unwrap()
                );
                assert_eq!(
                    run(quarter, printed(result, "0.1"), &inputs).status,
                    Status::PrecisionNotEstablished
                );
            }
        }
    }

    #[test]
    fn every_competing_pair_is_retained_and_a_matching_vintage_never_wins_a_tie() {
        let inputs = [
            observation("end-a", 2, "200"),
            observation("prior", 1, "100"),
            observation("end-b", 2, "250"),
        ];
        let report = run(2, printed("100", "1"), &inputs);
        assert_eq!(report.status, Status::ConflictingDerivations);
        assert_eq!(report.pairs.len(), 2);
        assert!(report.pairs.iter().any(|p| p.relation == Status::Derived));
        assert!(
            report
                .pairs
                .iter()
                .any(|p| p.relation == Status::Contradicted)
        );
        // Even a very coarse draft cannot make incompatible source assertions internally consistent.
        assert_eq!(
            run(2, printed("100", "1000"), &inputs).status,
            Status::ConflictingDerivations
        );
        let mut reversed = inputs.to_vec();
        reversed.reverse();
        assert_eq!(report, run(2, printed("100", "1"), &reversed));
    }

    #[test]
    fn missing_or_unusable_members_and_incompatible_series_refuse_positive_derivation() {
        let inputs = [observation("end", 2, "200"), observation("prior", 1, "100")];
        assert_eq!(
            run(2, printed("100", "1"), &inputs[..1]).status,
            Status::NotAvailable
        );
        for mutation in 0..5 {
            let mut changed = inputs.clone();
            match mutation {
                0 => changed[1].printed = None,
                1 => changed[1].series = "different-basis".into(),
                2 => changed[1].through_quarter = 4,
                3 => changed[1].through_quarter = u8::MAX,
                _ => changed[1].source_id = changed[0].source_id.clone(),
            }
            assert_eq!(
                run(2, printed("100", "1"), &changed).status,
                Status::Refused
            );
        }
        assert_eq!(
            run(2, printed("110", "1"), &inputs).status,
            Status::Contradicted
        );
        assert_eq!(run(2, printed("100", "0"), &inputs).status, Status::Refused);
        for quarter in [0, 1, 5, u8::MAX] {
            assert_eq!(
                run(quarter, printed("100", "1"), &inputs).status,
                Status::Refused
            );
        }
    }

    #[test]
    fn exact_overflow_is_a_refusal_with_no_success_status() {
        let mut inputs = [observation("end", 2, "200"), observation("prior", 1, "100")];
        inputs[0].printed = Some(Printed {
            value: Rat::from_integer(i128::MAX),
            step: Rat::from_integer(1),
        });
        let report = run(2, printed("100", "1"), &inputs);
        assert_eq!(report.status, Status::Refused);
        assert!(report.pairs.is_empty());
    }
}
