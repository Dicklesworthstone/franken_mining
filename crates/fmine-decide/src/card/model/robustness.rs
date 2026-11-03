//! DESIGN (BD-225, §22.5): source-bound declarations for finite-support Wasserstein comparisons.
//! Determinism: Exact (quoted rational controls and original matrix-row anchors).

use super::*;
use crate::card::robustness::{
    MAX_TRANSPORT_STATES, MAX_TRANSPORT_WORK, METHOD, RobustnessDesign, StateDistance,
};

#[derive(Default)]
pub(super) struct Rows {
    declaration: Option<(RobustnessDesign, usize)>,
    distances: BTreeMap<(String, String), (Rat, usize)>,
}

impl Rows {
    pub(super) fn declaration(&mut self, row: &TomlRow, model_key: &str) -> Result<(), Refusal> {
        if self.declaration.is_some() {
            return Err(bad(
                INPUT,
                row.line,
                "only one robustness declaration is permitted",
            ));
        }
        let [method, radius, metric_basis, radius_basis] = strings(
            &row.fields,
            ["method", "radius", "metric_basis", "radius_basis"],
            row.line,
            INPUT,
        )?;
        if method != METHOD {
            return Err(bad(
                INPUT,
                row.line,
                "robustness method must be wasserstein_1",
            ));
        }
        if metric_basis.len() > 4096 || radius_basis.len() > 4096 {
            return Err(bad(
                INPUT,
                row.line,
                "robustness bases must not exceed 4096 bytes",
            ));
        }
        self.declaration = Some((
            RobustnessDesign {
                model_key: model_key.into(),
                method: method.into(),
                radius: nonnegative_fraction(radius, row.line)?,
                metric_basis: metric_basis.into(),
                radius_basis: radius_basis.into(),
                distances: Vec::new(),
            },
            row.line,
        ));
        Ok(())
    }

    pub(super) fn distance(&mut self, row: &TomlRow) -> Result<(), Refusal> {
        let [from, to, value] = strings(&row.fields, ["from", "to", "value"], row.line, INPUT)?;
        for id in [from, to] {
            key(id, row.line)?;
            if id.len() > 128 {
                return Err(bad(
                    INPUT,
                    row.line,
                    "transport state identifiers exceed 128 bytes",
                ));
            }
        }
        if self.distances.len() >= MAX_TRANSPORT_STATES * MAX_TRANSPORT_STATES {
            return Err(bad(
                DOMAIN,
                row.line,
                "transport matrix exceeds 64 by 64 states",
            ));
        }
        if self
            .distances
            .insert(
                (from.into(), to.into()),
                (nonnegative_fraction(value, row.line)?, row.line),
            )
            .is_some()
        {
            return Err(bad(
                BASIS,
                row.line,
                &format!("duplicate transport distance {from}/{to}"),
            ));
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        states: &BTreeMap<String, Rat>,
        action_count: usize,
        unknowns: &[UuTerm],
        source: &SourceAnchor,
    ) -> Result<Option<Claim<RobustnessDesign>>, Refusal> {
        let Some((mut design, line)) = self.declaration else {
            if let Some((_, (_, line))) = self.distances.first_key_value() {
                return Err(bad(
                    INPUT,
                    *line,
                    "transport distances require a robustness declaration",
                ));
            }
            return Ok(None);
        };
        let work = states
            .len()
            .checked_mul(states.len())
            .and_then(|n| n.checked_mul(action_count))
            .and_then(|n| n.checked_mul(2));
        if states.is_empty()
            || states.len() > MAX_TRANSPORT_STATES
            || action_count > super::super::MAX_ACTIONS
            || work.is_none_or(|n| n > MAX_TRANSPORT_WORK)
        {
            return Err(bad(
                DOMAIN,
                line,
                "robustness requires at most 64 states, 256 actions and 2097152 state-pair/action evaluations",
            ));
        }
        for ((from, to), (_, distance_line)) in &self.distances {
            if !states.contains_key(from) || !states.contains_key(to) {
                return Err(bad(
                    BASIS,
                    *distance_line,
                    &format!("transport distance names unknown state {from}/{to}"),
                ));
            }
        }
        let mut anchors = vec![located(
            source,
            &format!("robustness method, radius and bases, line {line}"),
        )];
        for from in states.keys() {
            for to in states.keys() {
                let (distance, distance_line) = self.distances.get(&(from.clone(), to.clone()))
                    .ok_or_else(|| bad(BASIS, line, &format!("missing transport distance {from}/{to}; every ordered pair including the diagonal must be explicit")))?;
                design.distances.push(StateDistance {
                    from_state: from.clone(),
                    to_state: to.clone(),
                    distance: *distance,
                });
                anchors.push(located(
                    source,
                    &format!("transport distance {from}/{to}, line {distance_line}"),
                ));
            }
        }
        let tag = design.canonical_tag();
        Ok(Some(Claim::new(
            format!("decision.{}.robustness", design.model_key),
            design,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            unknowns.to_vec(),
            Vec::new(),
            anchors,
        )))
    }
}

fn nonnegative_fraction(value: &str, line: usize) -> Result<Rat, Refusal> {
    let fail = || {
        bad(
            DOMAIN,
            line,
            "transport radius and distance require a nonnegative exact fraction n/d with positive denominator",
        )
    };
    let (numerator, denominator) = value.split_once('/').ok_or_else(fail)?;
    if numerator.is_empty()
        || denominator.is_empty()
        || !numerator
            .bytes()
            .chain(denominator.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err(fail());
    }
    let numerator: i128 = numerator.parse().map_err(|_| fail())?;
    let denominator: i128 = denominator.parse().map_err(|_| fail())?;
    if denominator == 0 {
        return Err(fail());
    }
    Rat::new(numerator, denominator).map_err(|_| fail())
}
