//! DESIGN (BD-225, §22.1): bind complete forecast cohorts to a choice, then read their saved scores.
//! This connection supplies no automatic decision, model provenance or evaluation of decision quality.
//! Determinism: DeterministicPerIsa (exact references, inherited forecast score replay).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_core::{ObjectId, id::PrefixKind};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use super::{
    Digest32, Envelope, Journal, Refusal, RefusalCode, Snapshot, clean, digest, integrity, invalid,
    js, take,
};
use crate::forecast::live::decisions::{self, CohortReference, Commitments};

pub(super) const NO_CLAIM: &str = "A prospective decision's explicitly selected frozen forecast cohorts and their saved assessments. Complete memberships, scoring rules and the original choice remain unchanged. Scores describe the registered predictions, not the quality or causal effect of the decision. Relevance to the decision is an operator declaration; no automatic payoff mapping, independent model provenance, calibrated probabilities, human sign-off, execution authority or external time proof is supplied. Chronology uses the journals' operator-declared host-clock uncertainty bounds.";

/// Explicit complete cohorts from one currently pinned forecast history. Admission verifies their
/// identities and chronology while holding the history's shared lock through the decision append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastSelection {
    pub journal: PathBuf,
    pub head: Digest32,
    pub cohorts: Vec<String>,
}

impl ForecastSelection {
    pub(super) fn parse(fields: &mut BTreeMap<String, String>) -> Result<Option<Self>, String> {
        if ![
            "--forecast-journal",
            "--forecast-head",
            "--forecast-cohorts",
        ]
        .iter()
        .any(|key| fields.contains_key(*key))
        {
            return Ok(None);
        }
        Ok(Some(Self {
            journal: PathBuf::from(take(fields, "--forecast-journal")?),
            head: digest(&take(fields, "--forecast-head")?)?,
            cohorts: names(&take(fields, "--forecast-cohorts")?, 32)?,
        }))
    }
}

pub(super) fn names(value: &str, limit: usize) -> Result<Vec<String>, String> {
    let mut values = value
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if values.is_empty() || values.len() > limit || values.iter().any(|value| !clean(value, 256)) {
        return Err(format!(
            "expected one through {limit} nonempty bounded comma-separated identifiers"
        ));
    }
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("duplicate identifier in a complete selection".into());
    }
    Ok(values)
}

#[derive(Debug, Clone)]
pub(super) struct ForecastLinks {
    journal_path: String,
    commitments: Commitments,
}

impl ForecastLinks {
    pub(super) fn admit(
        selection: &ForecastSelection,
        root: &Path,
    ) -> Result<(Journal, Self), Refusal> {
        let journal_path = selection
            .journal
            .to_str()
            .filter(|path| clean(path, 4096))
            .ok_or_else(|| {
                invalid("forecast journal path must be bounded UTF-8 without control characters")
            })?
            .to_owned();
        let (journal, commitments) = decisions::admit(
            &root.join(&selection.journal),
            selection.head,
            &selection.cohorts,
        )?;
        let links = Self {
            journal_path,
            commitments,
        };
        links.validate()?;
        Ok((journal, links))
    }

    fn validate(&self) -> Result<(), Refusal> {
        let c = &self.commitments;
        if !clean(&self.journal_path, 4096)
            || !clean(&c.cell_id, 256)
            || c.head == Digest32::default()
            || c.sequence == 0
            || c.latest_ns < 0
            || c.cohorts.is_empty()
            || c.cohorts.len() > 32
        {
            return Err(integrity("invalid retained forecast journal commitment"));
        }
        for cohort in &c.cohorts {
            if !clean(&cohort.name, 256)
                || cohort.name.contains(',')
                || cohort.digest == Digest32::default()
                || cohort.frozen_at_ns < 0
                || cohort.frozen_at_ns > c.latest_ns
                || cohort.prospective_until_ns <= cohort.frozen_at_ns
                || cohort.members.is_empty()
                || cohort.members.len() > 4096
                || cohort
                    .members
                    .iter()
                    .any(|member| member.kind != PrefixKind::Forecast)
                || cohort.members.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(integrity("invalid retained complete forecast cohort"));
            }
        }
        if c.cohorts
            .windows(2)
            .any(|pair| pair[0].name >= pair[1].name)
        {
            return Err(integrity(
                "retained forecast cohorts must be distinct and canonically ordered",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_at(&self, earliest_ns: i64, latest_ns: i64) -> Result<(), Refusal> {
        self.validate()?;
        if self.commitments.latest_ns >= earliest_ns
            || self
                .commitments
                .cohorts
                .iter()
                .any(|cohort| latest_ns >= cohort.prospective_until_ns)
        {
            return Err(Refusal::new(
                RefusalCode::LateRegistration206,
                "decision forecast commitments must precede the decision's earliest possible time, and its latest possible time must precede every outcome deadline and plant support start",
            ));
        }
        Ok(())
    }

    pub(super) fn encode(&self) -> Result<String, Refusal> {
        self.validate()?;
        let c = &self.commitments;
        let mut out = String::new();
        let mut field = |key: &str, value: &str| {
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            let _ = writeln!(out, "{key} = \"{escaped}\"");
        };
        field("journal_path", &self.journal_path);
        field("cell_id", &c.cell_id);
        field("head", &c.head.to_hex());
        field("sequence", &c.sequence.to_string());
        field("latest_ns", &c.latest_ns.to_string());
        for cohort in &c.cohorts {
            let _ = writeln!(out, "[[cohort]]");
            for (key, value) in [
                ("name", cohort.name.clone()),
                ("digest", cohort.digest.to_hex()),
                ("frozen_at_ns", cohort.frozen_at_ns.to_string()),
                (
                    "prospective_until_ns",
                    cohort.prospective_until_ns.to_string(),
                ),
                (
                    "members",
                    cohort
                        .members
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ),
            ] {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                let _ = writeln!(out, "{key} = \"{escaped}\"");
            }
        }
        Ok(out)
    }

    pub(super) fn decode(text: &str) -> Result<Self, Refusal> {
        fn fields(fields: Vec<TomlField>) -> Result<BTreeMap<String, String>, Refusal> {
            fields
                .into_iter()
                .map(|field| match field.value {
                    TomlValue::Str(value) => Ok((field.key, value)),
                    _ => Err(integrity(
                        "retained forecast reference fields must be strings",
                    )),
                })
                .collect()
        }
        fn number<T: std::str::FromStr>(
            fields: &mut BTreeMap<String, String>,
            key: &str,
        ) -> Result<T, Refusal> {
            take(fields, key)
                .map_err(integrity)?
                .parse()
                .map_err(|_| integrity(format!("invalid retained forecast {key}")))
        }
        let doc = parse_subset(text).map_err(|error| integrity(error.to_string()))?;
        let mut top = fields(doc.top)?;
        let journal_path = take(&mut top, "journal_path").map_err(integrity)?;
        let mut commitments = Commitments {
            cell_id: take(&mut top, "cell_id").map_err(integrity)?,
            head: digest(&take(&mut top, "head").map_err(integrity)?).map_err(integrity)?,
            sequence: number(&mut top, "sequence")?,
            latest_ns: number(&mut top, "latest_ns")?,
            cohorts: Vec::new(),
        };
        if !top.is_empty() {
            return Err(integrity("unknown retained forecast reference field"));
        }
        for row in doc.rows {
            if row.table != "cohort" {
                return Err(integrity("unknown retained forecast reference table"));
            }
            let mut row = fields(row.fields)?;
            commitments.cohorts.push(CohortReference {
                name: take(&mut row, "name").map_err(integrity)?,
                digest: digest(&take(&mut row, "digest").map_err(integrity)?).map_err(integrity)?,
                frozen_at_ns: number(&mut row, "frozen_at_ns")?,
                prospective_until_ns: number(&mut row, "prospective_until_ns")?,
                members: take(&mut row, "members")
                    .map_err(integrity)?
                    .split(',')
                    .map(|value| {
                        value
                            .parse::<ObjectId>()
                            .map_err(|error| integrity(error.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            });
            if !row.is_empty() {
                return Err(integrity("unknown retained forecast cohort field"));
            }
        }
        let result = Self {
            journal_path,
            commitments,
        };
        if result.encode()? != text {
            return Err(integrity("noncanonical retained forecast commitments"));
        }
        Ok(result)
    }

    pub(super) fn json(&self) -> String {
        let c = &self.commitments;
        let cohorts = c
            .cohorts
            .iter()
            .map(cohort_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"journal_path\":{},\"cell_id\":{},\"prefix_head\":{},\"prefix_sequence\":{},\"prefix_latest_ns\":{},\"cohorts\":[{cohorts}]}}",
            js(&self.journal_path),
            js(&c.cell_id),
            js(&c.head.to_hex()),
            c.sequence,
            js(&c.latest_ns.to_string())
        )
    }
}

fn cohort_json(cohort: &CohortReference) -> String {
    format!(
        "{{\"name\":{},\"digest\":{},\"frozen_at_ns\":{},\"prospective_until_ns\":{},\"members\":[{}]}}",
        js(&cohort.name),
        js(&cohort.digest.to_hex()),
        js(&cohort.frozen_at_ns.to_string()),
        js(&cohort.prospective_until_ns.to_string()),
        cohort
            .members
            .iter()
            .map(|member| js(&member.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn review(
    journal: &Journal,
    snapshot: &Snapshot,
    decision_record_head: Digest32,
    forecast_path: &Path,
    forecast_head: Digest32,
    selected: &[Digest32],
) -> Result<Envelope, Refusal> {
    let links = snapshot.forecasts.as_ref().ok_or_else(|| invalid(
        "this decision has no prospectively committed forecasts; links cannot be attached after recording"))?;
    let assessments =
        decisions::assessments(forecast_path, forecast_head, &links.commitments, selected)?;
    let mut rows = Vec::new();
    let mut missing = 0;
    let mut markdown = format!(
        "# Forecast outcomes for decision {}\n\n{NO_CLAIM}\n\nOriginal choice: {}.\n\nRationale: {}\n\nDecision record head: `{decision_record_head}`. Forecast head: `{forecast_head}`.\n",
        snapshot.decision_id,
        snapshot.choice.description(),
        snapshot.rationale
    );
    for cohort in &links.commitments.cohorts {
        let selected = assessments
            .iter()
            .filter(|assessment| assessment.cohort == cohort.digest)
            .map(|assessment| assessment.report.clone())
            .collect::<Vec<_>>();
        let state = if selected.is_empty() {
            missing += 1;
            "no_assessment_selected"
        } else {
            "saved_assessments"
        };
        rows.push(format!(
            "{{\"cohort\":{},\"status\":{},\"assessments\":[{}]}}",
            cohort_json(cohort),
            js(state),
            selected.join(",")
        ));
        let _ = writeln!(
            markdown,
            "\n## {}\n\nCohort `{}`; {} committed predictions. Status: `{state}`.\n",
            cohort.name,
            cohort.digest,
            cohort.members.len()
        );
        for report in selected {
            let _ = writeln!(markdown, "```json\n{report}\n```\n");
        }
    }
    let mut envelope = Envelope::ok(
        "decide journal assess",
        format!(
            "{{\"schema\":\"fmine.decision-assessment.v1\",\"read_only\":true,\"decision_journal_head\":{},\"decision_record_head\":{},\"forecast_journal_head\":{},\"prefix_verified\":true,\"decision\":{},\"n_committed_cohorts\":{},\"n_selected_assessments\":{},\"n_cohorts_without_selected_assessment\":{missing},\"forecasts\":[{}]}}",
            js(&journal.head().to_hex()),
            js(&decision_record_head.to_hex()),
            js(&forecast_head.to_hex()),
            snapshot.json()?,
            links.commitments.cohorts.len(),
            assessments.len(),
            rows.join(",")
        ),
        Vec::new(),
    );
    envelope.no_claim = Some(NO_CLAIM);
    envelope.rendered_markdown = Some(markdown);
    Ok(envelope)
}
