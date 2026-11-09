//! DESIGN (BD-229): source-bound finite-support transport geometry and one declared stress radius.
//! Determinism: Exact (strict fractions, complete named state pairs and original source anchors).

use fmine_registry::toml_subset::TomlRow;

use super::*;
use crate::robustness::{MAX_TRANSPORT_STATES, METHOD, RobustnessDesign, StateDistance};

#[derive(Default)]
pub(super) struct Rows {
    declaration: Option<(RobustnessDesign, usize)>,
    distances: BTreeMap<(String, String), (Rat, usize)>,
}

fn nonnegative(value: &str, line: usize) -> Result<Rat> {
    let fail = || {
        input(
            line,
            "transport controls require an exact nonnegative n/d fraction",
        )
    };
    let (n, d) = value.split_once('/').ok_or_else(fail)?;
    if n.is_empty() || d.is_empty() || !n.bytes().chain(d.bytes()).all(|b| b.is_ascii_digit()) {
        return Err(fail());
    }
    let n: i128 = n.parse().map_err(|_| fail())?;
    let d: i128 = d.parse().map_err(|_| fail())?;
    if d <= 0 {
        return Err(fail());
    }
    Rat::new(n, d).map_err(|_| fail())
}

impl Rows {
    pub(super) fn push(&mut self, row: &TomlRow, model_key: &str) -> Result<()> {
        if row.table == "robustness" {
            if self.declaration.is_some() {
                return Err(input(
                    row.line,
                    "only one robustness declaration is allowed",
                ));
            }
            let f = fields(
                &row.fields,
                &["method", "radius", "metric_basis", "radius_basis"],
                &[],
                row.line,
            )?;
            if get(&f, "method")? != METHOD {
                return Err(input(
                    row.line,
                    "robustness requires method = wasserstein_1",
                ));
            }
            self.declaration = Some((
                RobustnessDesign {
                    model_key: model_key.into(),
                    method: METHOD.into(),
                    radius: nonnegative(get(&f, "radius")?, row.line)?,
                    metric_basis: get(&f, "metric_basis")?.into(),
                    radius_basis: get(&f, "radius_basis")?.into(),
                    distances: vec![],
                },
                row.line,
            ));
        } else {
            let f = fields(&row.fields, &["from", "to", "value"], &[], row.line)?;
            let from = get(&f, "from")?;
            let to = get(&f, "to")?;
            if !key(from) || !key(to) {
                return Err(input(
                    row.line,
                    "transport state names must be bounded identifiers",
                ));
            }
            if self.distances.len() >= MAX_TRANSPORT_STATES * MAX_TRANSPORT_STATES {
                return Err(input(row.line, "transport matrix exceeds 64 by 64 states"));
            }
            if self
                .distances
                .insert(
                    (from.into(), to.into()),
                    (nonnegative(get(&f, "value")?, row.line)?, row.line),
                )
                .is_some()
            {
                return Err(input(row.line, "duplicate transport distance"));
            }
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        states: &BTreeMap<String, Rat>,
        source: &SourceAnchor,
        top: &[TomlField],
        unknowns: &[UuTerm],
        uu_anchors: &[SourceAnchor],
    ) -> Result<Option<Claim<RobustnessDesign>>> {
        let Some((mut design, line)) = self.declaration else {
            if !self.distances.is_empty() {
                return Err(input(
                    1,
                    "transport distances require a robustness declaration",
                ));
            }
            return Ok(None);
        };
        if !(1..=MAX_TRANSPORT_STATES).contains(&states.len())
            || self.distances.len() != states.len() * states.len()
        {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                "capital transport needs 1..=64 states and every ordered pair, including diagonals",
            ));
        }
        let mut anchors = vec![located(
            source,
            line,
            "capital robustness metric, radius and bases",
        )];
        for ((from, to), (distance, row_line)) in self.distances {
            if !states.contains_key(&from) || !states.contains_key(&to) {
                return Err(input(row_line, "transport distance names an unknown state"));
            }
            anchors.push(located(
                source,
                row_line,
                &format!("transport distance {from}/{to}"),
            ));
            design.distances.push(StateDistance {
                from_state: from,
                to_state: to,
                distance,
            });
        }
        anchors.extend(
            top.iter()
                .filter(|field| field.key == "model_key")
                .map(|field| located(source, field.line, &field.key)),
        );
        anchors.extend(uu_anchors.iter().cloned());
        let tag = design.canonical_tag();
        Ok(Some(assumed(
            "capital.robustness/1".into(),
            design,
            &tag,
            AdditivityClass::NonAdditive,
            unknowns,
            anchors,
        )))
    }
}
