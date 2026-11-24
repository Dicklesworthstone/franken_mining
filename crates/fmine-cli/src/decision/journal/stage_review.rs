//! DESIGN (bd-sk59): dated observation of a frozen capital policy at its review boundary.
//! Determinism: Exact

use fmine_calib::record::{parse_date, parse_utc};
use fmine_io::omf::json::Json;

use super::{
    Choice, Digest32, History, Journal, Refusal, RefusalCode, Snapshot, clean, digest, integrity,
    invalid, js, parse_subset, sha256,
};
use crate::capital::staged_journal::retained_policy;
use fmine_registry::toml_subset::TomlValue;

const MODEL: &str = "fmine.capital-stage-observation.v1";
const PREFIX: &str = r#"{"schema":"fmine.capital-stage-review.v1","#;
const ACTION: &str = "observe-stage";
const NS_PER_SECOND: i128 = 1_000_000_000;
const NS_PER_DAY: i128 = 86_400 * NS_PER_SECOND;

pub(super) struct Observation {
    initial_decision: String,
    pub(super) initial_head: Digest32,
    observation: Option<String>,
    observed_at: String,
    available_at: String,
    source: String,
    evidence: String,
    observed_ns: i128,
    available_ns: i128,
}

pub(super) fn parse_if(text: &str) -> Result<Option<Observation>, Refusal> {
    let Ok(doc) = parse_subset(text) else {
        return Ok(None);
    };
    if !matches!(doc.top_get("schema").map(|f| &f.value), Some(TomlValue::Str(s)) if s == MODEL) {
        return Ok(None);
    }
    let names = [
        "schema",
        "initial_decision",
        "initial_head",
        "observation",
        "observed_at",
        "available_at",
        "source",
        "evidence",
    ];
    if !doc.rows.is_empty() || doc.top.iter().any(|f| !names.contains(&f.key.as_str())) {
        return Err(invalid(
            "stage observation requires only its declared top-level fields",
        ));
    }
    let field = |name: &str, max| -> Result<&str, Refusal> {
        match doc.top_get(name).map(|f| &f.value) {
            Some(TomlValue::Str(s)) if clean(s, max) && s.trim() == s => Ok(s),
            _ => Err(invalid(format!(
                "stage observation needs a bounded {name} string"
            ))),
        }
    };
    let observed_at = field("observed_at", 20)?;
    let available_at = field("available_at", 20)?;
    let evidence = field("evidence", 32)?;
    if !matches!(evidence, "synthetic" | "operator_declared") {
        return Err(invalid(
            "stage observations require synthetic or operator_declared evidence",
        ));
    }
    let timestamp = |value| {
        parse_utc(value)
            .map(|s| i128::from(s) * NS_PER_SECOND)
            .ok_or_else(|| {
                invalid("stage evidence requires UTC timestamps in YYYY-MM-DDTHH:MM:SSZ form")
            })
    };
    let observed_ns = timestamp(observed_at)?;
    let available_ns = timestamp(available_at)?;
    if observed_ns > available_ns {
        return Err(invalid(
            "stage evidence availability precedes its observation",
        ));
    }
    Ok(Some(Observation {
        initial_decision: field("initial_decision", 256)?.to_owned(),
        initial_head: digest(field("initial_head", 64)?).map_err(invalid)?,
        observation: doc
            .top_get("observation")
            .map(|_| field("observation", 128).map(str::to_owned))
            .transpose()?,
        observed_at: observed_at.to_owned(),
        available_at: available_at.to_owned(),
        source: field("source", 4096)?.to_owned(),
        evidence: evidence.to_owned(),
        observed_ns,
        available_ns,
    }))
}

pub(super) fn retained(snapshot: &Snapshot) -> Result<Option<Observation>, Refusal> {
    let review_card = snapshot.card.starts_with(PREFIX);
    if !review_card && !matches!(&snapshot.choice, Choice::ActNow(action) if action == ACTION) {
        return Ok(None);
    }
    let model = parse_if(&snapshot.model)?;
    if model.is_some() != review_card {
        return Err(integrity(
            "stage observation worksheet and retained review disagree",
        ));
    }
    Ok(model)
}

fn text<'a>(record: &'a Json, name: &str) -> Result<&'a str, Refusal> {
    record
        .get(name)
        .and_then(Json::as_str)
        .ok_or_else(|| integrity(format!("retained initial policy lacks {name}")))
}

impl Observation {
    pub(super) fn available_at(&self, earliest: i64) -> Result<(), Refusal> {
        if self.available_ns > i128::from(earliest) {
            return Err(invalid(
                "stage evidence must be available by the earliest possible recording time (UTC)",
            ));
        }
        Ok(())
    }

    pub(super) fn prepare(
        &self,
        journal: &Journal,
        history: &History,
        choice: &Choice,
        model: &str,
    ) -> Result<(String, String), Refusal> {
        if !matches!(choice, Choice::ActNow(action) if action == ACTION) {
            return Err(invalid(
                "stage observation worksheets require --action observe-stage",
            ));
        }
        let index = history
            .ids
            .get(&self.initial_decision)
            .copied()
            .ok_or_else(|| {
                integrity(
                    "the pinned initial decision must precede the observation in this journal",
                )
            })?;
        let record = journal
            .records()
            .get(index + 1)
            .ok_or_else(|| integrity("initial policy record is absent"))?;
        if record.chain_head()? != self.initial_head {
            return Err(integrity(
                "initial_head differs from the original initial policy record",
            ));
        }
        if history.stage_reviews.contains(&self.initial_head) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                "this initial policy already has a retained stage observation",
            ));
        }
        let initial = history
            .snapshots
            .get(index)
            .ok_or_else(|| integrity("initial policy snapshot is absent"))?;
        let policy_card = retained_policy(&initial.card).ok_or_else(|| {
            Refusal::new(
                RefusalCode::BasisMismatch105,
                "the pinned decision is not an initial staged capital policy",
            )
        })??;
        let observation_date = text(&policy_card, "observation_date_utc")?;
        let cutoff = parse_date(observation_date)
            .ok_or_else(|| integrity("invalid retained observation date"))?;
        if self.observed_ns < i128::from(cutoff) * NS_PER_DAY {
            return Err(invalid(
                "stage observation precedes the frozen policy's review date (UTC)",
            ));
        }
        let policy = policy_card
            .get("policy")
            .ok_or_else(|| integrity("retained policy is absent"))?;
        let acquired = policy
            .get("information_acquired")
            .and_then(Json::as_bool)
            .ok_or_else(|| integrity("retained policy lacks its information gate"))?;
        if acquired != self.observation.is_some() {
            return Err(invalid(
                "declare an observation exactly when the frozen policy bought the information gate",
            ));
        }
        let branches = policy
            .get("branches")
            .and_then(Json::as_array)
            .filter(|branches| !branches.is_empty() && (acquired || branches.len() == 1))
            .ok_or_else(|| integrity("retained policy lacks the declared branches"))?;
        let mut branch = None;
        for candidate in branches {
            let label = match candidate.get("observation_id") {
                Some(Json::Null) if !acquired => None,
                Some(Json::String(id)) if acquired && !id.is_empty() => Some(id.as_str()),
                _ => {
                    return Err(integrity(
                        "retained branch does not match the policy's information gate",
                    ));
                }
            };
            if self.observation.as_deref() == label && branch.replace(candidate).is_some() {
                return Err(integrity(
                    "retained policy has more than one branch for this observation",
                ));
            }
        }
        let modeled = match branch {
            None => "null".to_owned(),
            Some(branch) => {
                let actions = branch
                    .get("selected")
                    .and_then(Json::as_array)
                    .ok_or_else(|| integrity("retained branch lacks selected actions"))?;
                let names = actions
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(js)
                            .ok_or_else(|| integrity("retained branch has an invalid action"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                format!(
                    r#"{{"selected":[{}],"spend":{},"spending_unit":"USD"}}"#,
                    names.join(","),
                    js(text(branch, "spend")?)
                )
            }
        };
        let signal = self
            .observation
            .as_deref()
            .map_or_else(|| "null".into(), js);
        let result = match (acquired, branch.is_some()) {
            (true, true) => "matched_branch",
            (true, false) => "outside_policy",
            (false, _) => "common_continuation",
        };
        let card = format!(
            r#"{PREFIX}"initial_decision":{},"initial_head":{},"initial_policy_claim_id":{},"comparison_digest":{},"policy_id":{},"observation_date_utc":{},"observation":{signal},"observed_at":{},"available_at":{},"source":{},"evidence":{},"observation_model_sha256":{},"result":"{result}","modeled_continuation":{modeled},"reserve_disclosure_claim":false,"execution_authorized":false,"geotechnical_signoff":false,"regulatory_compliance_claim":false}}"#,
            js(&self.initial_decision),
            js(&self.initial_head.to_hex()),
            js(&initial.claim_id),
            js(text(&policy_card, "comparison_digest")?),
            js(text(policy, "policy_id")?),
            js(observation_date),
            js(&self.observed_at),
            js(&self.available_at),
            js(&self.source),
            js(&self.evidence),
            js(&sha256(model.as_bytes()).to_hex()),
        );
        Ok((card, initial.claim_id.clone()))
    }
}
