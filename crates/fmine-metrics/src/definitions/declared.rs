//! Common admission, exact parsing and JSON primitives for declared metric worksheets.
//! Determinism: Exact (callers retain their domain rules, refusal prefixes and receipt schemas).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::color::validate_color_payload;
use fmine_evidence::{
    Color, EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::toml_subset::{TomlField, TomlValue};
use fmine_units::ExactQty;

use crate::guidance::{Ownership, Period};

/// Immutable admitted worksheet inputs and their exact derived result.
#[derive(Debug, Clone, PartialEq)]
pub struct Receipt<I, O> {
    inputs: Vec<Claim<I>>,
    claim: Claim<O>,
    digest: String,
}

impl<I, O> Receipt<I, O> {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<I>] {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &O {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<O> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub(super) fn from_admitted(
        inputs: Vec<Claim<I>>,
        metric: &str,
        value: O,
        digest: String,
        additivity: AdditivityClass,
        bad: fn(RefusalCode, &str) -> Refusal,
    ) -> Result<Self, Refusal> {
        let mut evidence = EvidenceClass::Derived;
        let mut epistemic = EpistemicStatus::KnownKnown;
        let mut state = KnowledgeState::Known;
        let mut confidentiality = Confidentiality::public();
        let mut uu = BTreeMap::new();
        let mut parents = BTreeSet::new();
        let mut anchors = BTreeSet::new();
        for input in &inputs {
            evidence = evidence.meet(input.evidence);
            epistemic = epistemic.and(input.epistemic);
            state = state.and(input.state);
            confidentiality = confidentiality.join(&input.confidentiality);
            parents.insert(input.claim_id);
            anchors.extend(input.anchors.iter().cloned());
            for term in &input.uu_terms {
                if let Some(prior) = uu.insert(term.scenario_id.clone(), term.clone())
                    && prior != *term
                {
                    return Err(bad(
                        RefusalCode::UnbudgetedUnknowns204,
                        "conflicting definitions for a shared UU scenario",
                    ));
                }
            }
        }
        let claim = Claim::new(
            metric,
            value,
            &digest,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            evidence,
            epistemic,
            state,
            confidentiality,
            uu.into_values().collect(),
            parents.into_iter().collect(),
            anchors.into_iter().collect(),
        );
        Ok(Self {
            inputs,
            claim,
            digest,
        })
    }
}

pub(super) fn admit<T>(
    claim: &Claim<T>,
    metric: &str,
    tag: &str,
    additivity: AdditivityClass,
    named: fn(&str) -> bool,
    bad: fn(RefusalCode, &str) -> Refusal,
) -> Result<(), Refusal> {
    claim.check_id()?;
    if claim.metric_key != metric || claim.value_tag() != tag {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "input differs from its bound canonical value",
        ));
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale input"));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || claim.calibration_failed
    {
        return Err(bad(RefusalCode::NotObservable202, "unusable input state"));
    }
    if claim.representation != Representation::Deterministic
        || claim.support != SpatialSupport::PeriodAggregate
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "explicit deterministic period aggregates required",
        ));
    }
    if claim.additivity != additivity {
        return Err(bad(
            RefusalCode::NonAdditiveAverage103,
            "input additivity differs from its declared role",
        ));
    }
    if claim.anchors.is_empty()
        || claim
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "every input requires a source hash and locator",
        ));
    }
    if claim.uu_terms.iter().any(|u| {
        !named(&u.scenario_id)
            || !named(&u.description)
            || !named(&u.trigger_indicator)
            || !u.downside_exposure_usd_m.is_finite()
            || u.downside_exposure_usd_m < 0.0
    }) {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "invalid supplied UU metadata",
        ));
    }
    if let Ok(color) = claim.color() {
        validate_color_payload(&color).map_err(|_| {
            bad(
                RefusalCode::CalibrationRequired205,
                "invalid input colour metadata",
            )
        })?;
    }
    Ok(())
}

/// Declared worksheet bytes establish provenance, not observation or calibration.
pub(super) fn source_claim<T>(
    metric: &str,
    value: T,
    tag: &str,
    additivity: AdditivityClass,
    uu: Vec<UuTerm>,
    anchors: Vec<SourceAnchor>,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Known,
        Confidentiality::internal(),
        uu,
        Vec::new(),
        anchors,
    )
}

pub(super) fn decimal_parts(value: &str) -> Option<(bool, &str, &str)> {
    let (negative, digits) = value
        .strip_prefix('-')
        .map_or((false, value), |s| (true, s));
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fraction.is_empty())
    {
        return None;
    }
    Some((negative, whole, fraction))
}

pub(super) fn quantity(value: &str) -> Option<ExactQty> {
    if let Some((n, d)) = value.split_once('/') {
        let n = n.parse::<i128>().ok()?;
        let d = d.parse::<i128>().ok()?;
        (d > 0).then(|| ExactQty::new(n, d).ok()).flatten()
    } else {
        decimal_parts(value)?;
        ExactQty::from_decimal_str(value).ok()
    }
}

pub(super) fn period(value: &str) -> Option<Period> {
    if !value.is_ascii()
        || value.len() != 6
        || !value.as_bytes()[..4].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let year = value[..4].parse::<u16>().ok()?;
    let (first_month, last_month) = match &value[4..] {
        "FY" => (1, 12),
        "Q1" => (1, 3),
        "Q2" => (4, 6),
        "Q3" => (7, 9),
        "Q4" => (10, 12),
        "H1" => (1, 6),
        "H2" => (7, 12),
        _ => return None,
    };
    (year > 0).then_some(Period {
        year,
        first_month,
        last_month,
    })
}

pub(super) fn js(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(super) fn qty(value: ExactQty) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

pub(super) fn ownership_json(ownership: Ownership) -> String {
    match ownership {
        Ownership::Full => "{\"kind\":\"full\"}".into(),
        Ownership::Attributable { share } => format!(
            "{{\"kind\":\"attributable\",\"share\":{}}}",
            js(&qty(share))
        ),
    }
}

pub(super) fn field_anchors(
    source: &SourceAnchor,
    fields: &[TomlField],
    table: &str,
) -> Vec<SourceAnchor> {
    fields
        .iter()
        .map(|field| {
            SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {}: {table}.{}", field.line, field.key),
            )
        })
        .collect()
}

/// The same canonical bytes, source anchors and ordered parents under each receipt's existing keys.
pub(super) fn json_parts<T>(claim: &Claim<T>) -> (String, String, String) {
    let mut body = String::new();
    for byte in claim.canonical_body() {
        let _ = write!(body, "{byte:02x}");
    }
    let anchors = join_json(&claim.anchors, |a| {
        format!(
            "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
            js(&a.artifact_path),
            js(&Digest32(a.artifact_sha256).to_hex()),
            js(&a.locator)
        )
    });
    let parents = join_json(&claim.parents, |id| js(&id.to_string()));
    (body, anchors, parents)
}

/// Keeps each existing receipt's field order and UU spelling, while sharing its complete Claim envelope.
pub(super) fn claim_json<T>(claim: &Claim<T>, value: &str, cost_schema: bool) -> String {
    let (body, anchors, parents) = json_parts(claim);
    let uu = join_json(&claim.uu_terms, |u| {
        if cost_schema {
            format!(
                "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"downside_bits\":\"{:016x}\",\"trigger_indicator\":{}}}",
                js(&u.scenario_id),
                js(&u.description),
                u.downside_exposure_usd_m,
                u.downside_exposure_usd_m.to_bits(),
                js(&u.trigger_indicator)
            )
        } else {
            format!(
                "{{\"id\":{},\"description\":{},\"downside_usd_m_bits\":\"{:016x}\",\"trigger\":{}}}",
                js(&u.scenario_id),
                js(&u.description),
                u.downside_exposure_usd_m.to_bits(),
                js(&u.trigger_indicator)
            )
        }
    });
    let head = format!(
        "\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"value\":{value},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"representation\":{},\"support\":{},\"additivity\":{},\"confidentiality\":{}",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&body),
        js(&format!("{:?}", claim.evidence)),
        js(&format!("{:?}", claim.epistemic)),
        js(&format!("{:?}", claim.state)),
        js(&format!("{:?}", claim.representation)),
        js(&format!("{:?}", claim.support)),
        js(&format!("{:?}", claim.additivity)),
        js(&claim.confidentiality.to_string())
    );
    let supersedes = claim
        .supersedes
        .map_or_else(|| "null".into(), |id| js(&id.to_string()));
    let lineage = format!("\"parents\":[{parents}],\"anchors\":[{anchors}],\"uu_terms\":[{uu}]");
    let tail = if cost_schema {
        format!(
            "{lineage},\"supersedes\":{supersedes},\"stale\":{},\"calibration_failed\":{}",
            claim.stale, claim.calibration_failed
        )
    } else {
        format!(
            "\"stale\":{},\"calibration_failed\":{},\"supersedes\":{supersedes},{lineage}",
            claim.stale, claim.calibration_failed
        )
    };
    format!(
        "{{{head},{tail},\"color\":{}}}",
        color_json(claim.color(), cost_schema)
    )
}

pub(super) fn color_json(color: Result<Color, Refusal>, include_status: bool) -> String {
    let status = if include_status {
        "\"status\":\"available\","
    } else {
        ""
    };
    match color {
        Err(refusal) => {
            let key = if include_status {
                "refusal_code"
            } else {
                "code"
            };
            format!("{{\"status\":\"refused\",\"{key}\":{}}}", js(refusal.code))
        }
        Ok(Color::Verified { lo, hi }) => format!(
            "{{{status}\"kind\":\"verified\",\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
            lo.to_bits(),
            hi.to_bits()
        ),
        Ok(Color::Validated { regime, dataset }) => {
            let bounds = join_json(regime.bounds(), |(axis, (lo, hi))| {
                format!(
                    "{{\"axis\":{},\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
                    js(axis),
                    lo.to_bits(),
                    hi.to_bits()
                )
            });
            format!(
                "{{{status}\"kind\":\"validated\",\"dataset\":{},\"bounds\":[{bounds}]}}",
                js(&dataset)
            )
        }
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => format!(
            "{{{status}\"kind\":\"estimated\",\"estimator\":{},\"dispersion_bits\":\"{:016x}\"}}",
            js(&estimator),
            dispersion.to_bits()
        ),
    }
}

pub(super) type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

pub(super) fn fields<'a>(
    values: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
    bad: impl Fn(usize, String) -> Refusal,
) -> Result<Fields<'a>, Refusal> {
    let mut out = BTreeMap::new();
    for field in values {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings".into()));
        };
        if value.is_empty()
            || value.len() > 4096
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(bad(
                field.line,
                "values must be nonempty trimmed text of at most 4096 bytes".into(),
            ));
        }
        if out
            .insert(field.key.as_str(), (value.as_str(), field.line))
            .is_some()
        {
            return Err(bad(field.line, format!("duplicate field {}", field.key)));
        }
    }
    for name in required {
        if !out.contains_key(name) {
            return Err(bad(line, format!("missing {name}")));
        }
    }
    Ok(out)
}

pub(super) fn join_json<T>(
    values: impl IntoIterator<Item = T>,
    render: impl FnMut(T) -> String,
) -> String {
    values.into_iter().map(render).collect::<Vec<_>>().join(",")
}
