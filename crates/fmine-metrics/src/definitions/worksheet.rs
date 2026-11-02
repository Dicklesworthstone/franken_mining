//! Strict declared cost worksheets and their exact, evidence-preserving receipts (BD-061).
//! Determinism: Exact (monetary inputs never pass through floating point).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport, UuTerm};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{
    Color, EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::metrics::MetricRegistry;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::{Currency, ExactQty, Money};

use super::claims::{self, CostInput, CostReceipt, CostValue, Scope};
use crate::guidance::{Ownership, Period};

pub const SCHEMA: &str = "fmine.cost-worksheet.v1";
const MAX_ROWS: usize = 4096;
const MAX_FIELD_BYTES: usize = 4096;
type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn bad(line: usize, message: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("cost worksheet line {line}: {}", message.as_ref()),
    )
}

fn fields<'a>(
    fields: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>, Refusal> {
    let mut out = BTreeMap::new();
    for field in fields {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if value.len() > MAX_FIELD_BYTES
            || value.trim().is_empty()
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(bad(
                field.line,
                "values must be nonempty trimmed text of at most 4096 bytes",
            ));
        }
        out.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for name in required {
        if !out.contains_key(name) {
            return Err(bad(line, format!("missing {name}")));
        }
    }
    Ok(out)
}

fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize), Refusal> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}

fn decimal_parts(value: &str) -> Option<(bool, &str, &str)> {
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

fn quantity(value: &str, line: usize) -> Result<ExactQty, Refusal> {
    let fail = || {
        bad(
            line,
            "expected an exact decimal or n/d with a positive denominator",
        )
    };
    if let Some((n, d)) = value.split_once('/') {
        let n = n.parse::<i128>().map_err(|_| fail())?;
        let d = d.parse::<i128>().map_err(|_| fail())?;
        if d <= 0 {
            return Err(fail());
        }
        ExactQty::new(n, d).map_err(|_| fail())
    } else {
        decimal_parts(value).ok_or_else(fail)?;
        ExactQty::from_decimal_str(value).map_err(|_| fail())
    }
}

fn money(value: &str, line: usize) -> Result<Money, Refusal> {
    let fail = || {
        bad(
            line,
            "USD must be a plain decimal with at most four fractional digits that fits Money",
        )
    };
    let (negative, whole, fraction) = decimal_parts(value).ok_or_else(fail)?;
    if fraction.len() > 4 {
        return Err(fail());
    }
    let mut minor = 0_i128;
    for digit in whole
        .bytes()
        .chain(fraction.bytes())
        .chain(std::iter::repeat_n(b'0', 4 - fraction.len()))
    {
        let digit = i128::from(digit - b'0');
        minor = minor
            .checked_mul(10)
            .and_then(|n| {
                if negative {
                    n.checked_sub(digit)
                } else {
                    n.checked_add(digit)
                }
            })
            .ok_or_else(fail)?;
    }
    Ok(Money::from_minor(Currency::USD, minor))
}

fn period(value: &str, line: usize) -> Result<Period, Refusal> {
    let fail = || bad(line, "period must be YYYYFY, YYYYQ1..Q4 or YYYYH1..H2");
    if !value.is_ascii()
        || value.len() != 6
        || !value.as_bytes()[..4].iter().all(u8::is_ascii_digit)
    {
        return Err(fail());
    }
    let year = value[..4].parse::<u16>().map_err(|_| fail())?;
    let (first_month, last_month) = match &value[4..] {
        "FY" => (1, 12),
        "Q1" => (1, 3),
        "Q2" => (4, 6),
        "Q3" => (7, 9),
        "Q4" => (10, 12),
        "H1" => (1, 6),
        "H2" => (7, 12),
        _ => return Err(fail()),
    };
    if year == 0 {
        return Err(fail());
    }
    Ok(Period {
        year,
        first_month,
        last_month,
    })
}

fn anchored(source: &SourceAnchor, locator: impl Into<String>) -> SourceAnchor {
    SourceAnchor::new(&source.artifact_path, source.artifact_sha256, locator)
}

/// Parse authenticated worksheet and registry bytes, then evaluate their selected registered rule.
/// All declared inputs stay Assumed and Internal. `complete = "true"` is an explicit assumption,
/// never independent evidence that an omitted registered component is zero.
pub fn evaluate(
    text: &str,
    source: SourceAnchor,
    metrics_text: &str,
    metrics_source: SourceAnchor,
    definition: &str,
) -> Result<CostReceipt, Refusal> {
    for (bytes, anchor) in [(text, &source), (metrics_text, &metrics_source)] {
        if sha256(bytes.as_bytes()).0 != anchor.artifact_sha256 {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                "cost worksheet or metric registry bytes differ from their source SHA-256",
            ));
        }
    }
    let registry =
        MetricRegistry::parse(metrics_text).map_err(|e| bad(1, format!("metric registry: {e}")))?;
    let definition_tag = claims::definition_tag(&registry, definition)?;
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if doc.rows.len() > MAX_ROWS {
        return Err(bad(1, "worksheet permits at most 4096 cost/UU rows"));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "issuer",
            "subject",
            "period",
            "ownership",
            "basis",
            "currency",
            "complete",
            "ounces_sold",
        ],
        &["ownership_share", "gold_share"],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "currency")?.0 != "USD" {
        return Err(bad(
            get(&top, "currency")?.1,
            "currency must be USD; no implicit FX",
        ));
    }
    let (complete, complete_line) = get(&top, "complete")?;
    if complete != "true" {
        return Err(bad(complete_line, "complete must explicitly be \"true\""));
    }
    let ownership = match (get(&top, "ownership")?.0, top.get("ownership_share")) {
        ("full", None) => Ownership::Full,
        ("attributable", Some((share, line))) => Ownership::Attributable {
            share: quantity(share, *line)?,
        },
        _ => {
            return Err(bad(
                get(&top, "ownership")?.1,
                "ownership requires full without ownership_share, or attributable with an exact ownership_share",
            ));
        }
    };
    let (period_text, period_line) = get(&top, "period")?;
    let scope = Scope {
        issuer: get(&top, "issuer")?.0.into(),
        subject: get(&top, "subject")?.0.into(),
        period: period(period_text, period_line)?,
        ownership,
        basis: get(&top, "basis")?.0.into(),
    };
    let mut values = Vec::new();
    let (sold, line) = get(&top, "ounces_sold")?;
    values.push((CostValue::SoldOunces(quantity(sold, line)?), line));
    if let Some((share, line)) = top.get("gold_share") {
        values.push((CostValue::GoldShare(quantity(share, *line)?), *line));
    }
    let mut unknowns = BTreeMap::new();
    for row in &doc.rows {
        match row.table.as_str() {
            "cost" => {
                let f = fields(&row.fields, &["item", "usd"], &[], row.line)?;
                let (usd, line) = get(&f, "usd")?;
                values.push((
                    CostValue::Line {
                        item: get(&f, "item")?.0.into(),
                        amount: money(usd, line)?,
                    },
                    row.line,
                ));
            }
            "uu" => {
                let f = fields(
                    &row.fields,
                    &["id", "description", "trigger", "downside_usd_m"],
                    &[],
                    row.line,
                )?;
                let (exposure, line) = get(&f, "downside_usd_m")?;
                decimal_parts(exposure).ok_or_else(|| {
                    bad(
                        line,
                        "UU exposure must be a positive finite decimal in USD millions",
                    )
                })?;
                let exposure: f64 = exposure
                    .parse()
                    .map_err(|_| bad(line, "invalid UU exposure"))?;
                if !exposure.is_finite() || exposure <= 0.0 {
                    return Err(bad(line, "UU exposure must be positive and finite"));
                }
                let id = get(&f, "id")?.0;
                if unknowns
                    .insert(
                        id,
                        UuTerm {
                            scenario_id: id.into(),
                            description: get(&f, "description")?.0.into(),
                            trigger_indicator: get(&f, "trigger")?.0.into(),
                            downside_exposure_usd_m: exposure,
                        },
                    )
                    .is_some()
                {
                    return Err(bad(row.line, format!("duplicate UU scenario {id}")));
                }
            }
            other => return Err(bad(row.line, format!("unknown table {other}"))),
        }
    }
    values.push((
        CostValue::Rule {
            definition: definition.into(),
            definition_tag,
            complete: true,
        },
        complete_line,
    ));
    let uu: Vec<_> = unknowns.into_values().collect();
    let inputs = values
        .into_iter()
        .map(|(value, line)| {
            let mut anchors = vec![anchored(
                &source,
                format!("line {line}: declared cost worksheet input"),
            )];
            let rule = matches!(value, CostValue::Rule { .. });
            if rule {
                anchors.push(anchored(
                    &metrics_source,
                    format!("definition {definition} and inherited lines/exclusions"),
                ));
            }
            let input = CostInput {
                scope: scope.clone(),
                value,
            };
            let tag = input.canonical_tag();
            let additivity = input.additivity();
            Claim::new(
                claims::INPUT_METRIC,
                input,
                &tag,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                additivity,
                EvidenceClass::Assumed,
                EpistemicStatus::KnownUnknown(KuKind::Parametric),
                KnowledgeState::Known,
                Confidentiality::internal(),
                // Scope-wide unknowns belong to the completeness declaration and propagate once.
                // Copying the whole budget onto each line would multiply input/output size quadratically.
                if rule { uu.clone() } else { Vec::new() },
                Vec::new(),
                anchors,
            )
        })
        .collect::<Vec<_>>();
    claims::evaluate(&registry, &inputs)
}

fn js(value: &str) -> String {
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

fn qty(value: ExactQty) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn scope_json(scope: &Scope) -> String {
    let ownership = match scope.ownership {
        Ownership::Full => "{\"kind\":\"full\"}".into(),
        Ownership::Attributable { share } => format!(
            "{{\"kind\":\"attributable\",\"share\":{}}}",
            js(&qty(share))
        ),
    };
    format!(
        "{{\"issuer\":{},\"subject\":{},\"period\":{{\"year\":{},\"first_month\":{},\"last_month\":{}}},\"ownership\":{ownership},\"basis\":{}}}",
        js(&scope.issuer),
        js(&scope.subject),
        scope.period.year,
        scope.period.first_month,
        scope.period.last_month,
        js(&scope.basis)
    )
}

fn input_json(input: &CostInput) -> String {
    let value = match &input.value {
        CostValue::Line { item, amount } => format!(
            "{{\"kind\":\"cost\",\"item\":{},\"currency\":\"USD\",\"minor\":{},\"scale\":10000}}",
            js(item),
            js(&amount.minor.to_string())
        ),
        CostValue::SoldOunces(q) => {
            format!("{{\"kind\":\"ounces_sold\",\"value\":{}}}", js(&qty(*q)))
        }
        CostValue::GoldShare(q) => {
            format!("{{\"kind\":\"gold_share\",\"value\":{}}}", js(&qty(*q)))
        }
        CostValue::Rule {
            definition,
            definition_tag,
            complete,
        } => format!(
            "{{\"kind\":\"rule\",\"definition\":{},\"definition_tag\":{},\"complete\":{complete}}}",
            js(definition),
            js(definition_tag)
        ),
    };
    format!(
        "{{\"scope\":{},\"value\":{value}}}",
        scope_json(&input.scope)
    )
}

fn color_json(color: Result<Color, Refusal>) -> String {
    match color {
        Err(refusal) => format!(
            "{{\"status\":\"refused\",\"refusal_code\":{}}}",
            js(refusal.code)
        ),
        Ok(Color::Verified { lo, hi }) => format!(
            "{{\"status\":\"available\",\"kind\":\"verified\",\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
            lo.to_bits(),
            hi.to_bits()
        ),
        Ok(Color::Validated { regime, dataset }) => {
            let bounds = regime
                .bounds()
                .iter()
                .map(|(axis, (lo, hi))| {
                    format!(
                        "{{\"axis\":{},\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
                        js(axis),
                        lo.to_bits(),
                        hi.to_bits()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"status\":\"available\",\"kind\":\"validated\",\"dataset\":{},\"bounds\":[{bounds}]}}",
                js(&dataset)
            )
        }
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => format!(
            "{{\"status\":\"available\",\"kind\":\"estimated\",\"estimator\":{},\"dispersion_bits\":\"{:016x}\"}}",
            js(&estimator),
            dispersion.to_bits()
        ),
    }
}

fn claim_json<T>(claim: &Claim<T>, value: &str) -> String {
    let mut body = String::new();
    for byte in claim.canonical_body() {
        let _ = write!(body, "{byte:02x}");
    }
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&Digest32(a.artifact_sha256).to_hex()),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|u| format!("{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"downside_bits\":\"{:016x}\",\"trigger_indicator\":{}}}",
        js(&u.scenario_id), js(&u.description), u.downside_exposure_usd_m, u.downside_exposure_usd_m.to_bits(), js(&u.trigger_indicator))).collect::<Vec<_>>().join(",");
    format!(
        "{{\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"value\":{value},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"representation\":{},\"support\":{},\"additivity\":{},\"confidentiality\":{},\"parents\":[{}],\"anchors\":[{anchors}],\"uu_terms\":[{uu}],\"supersedes\":{},\"stale\":{},\"calibration_failed\":{},\"color\":{}}}",
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
        js(&claim.confidentiality.to_string()),
        claim
            .parents
            .iter()
            .map(|id| js(&id.to_string()))
            .collect::<Vec<_>>()
            .join(","),
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        claim.stale,
        claim.calibration_failed,
        color_json(claim.color())
    )
}

impl CostReceipt {
    /// Complete admitted Claims and exact values, including excluded inputs and named unknowns.
    #[must_use]
    pub fn to_json(&self) -> String {
        let s = self.summary();
        let contributions = s.contributions.iter().map(|c| format!("{{\"item\":{},\"input_claim_id\":{},\"amount_minor\":{},\"treatment\":{},\"numerator_usd\":{}}}",
            js(&c.item), js(&c.input_claim_id.to_string()), js(&c.amount.minor.to_string()), js(c.treatment.as_str()), js(&qty(c.numerator_usd))))
            .collect::<Vec<_>>().join(",");
        let summary = format!(
            "{{\"scope\":{},\"definition\":{},\"definition_tag\":{},\"superseded_by\":{},\"declared_complete\":{},\"currency\":\"USD\",\"numerator_usd\":{},\"ounces_sold\":{},\"gold_share\":{},\"usd_per_oz\":{},\"contributions\":[{contributions}]}}",
            scope_json(&s.scope),
            js(&s.definition),
            js(&s.definition_tag),
            s.superseded_by
                .as_ref()
                .map_or_else(|| "null".into(), |id| js(id)),
            s.declared_complete,
            js(&qty(s.numerator_usd)),
            js(&qty(s.ounces_sold)),
            s.gold_share.map_or_else(|| "null".into(), |q| js(&qty(q))),
            js(&qty(s.usd_per_oz))
        );
        format!(
            "{{\"digest\":{},\"summary\":{summary},\"claim\":{},\"inputs\":[{}]}}",
            js(self.digest()),
            claim_json(self.claim(), &summary),
            self.inputs()
                .iter()
                .map(|c| claim_json(c, &input_json(&c.value)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    /// Readable reconciliation with the same exact numbers and source identities as JSON.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let text = |v: &str| {
            v.replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace('`', "\\`")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let ownership = match s.scope.ownership {
            Ownership::Full => "100%".into(),
            Ownership::Attributable { share } => format!("attributable {}", qty(share)),
        };
        let mut out = format!(
            "# Declared cost worksheet\n\n{} / {} — {} months {}–{}, {}.\n\nBasis: {}.\n\n**{} USD/oz sold**, under `{}`.\n\nNumerator: {} USD; denominator: {} gold ounces sold.\n\n",
            text(&s.scope.issuer),
            text(&s.scope.subject),
            s.scope.period.year,
            s.scope.period.first_month,
            s.scope.period.last_month,
            ownership,
            text(&s.scope.basis),
            qty(s.usd_per_oz),
            text(&s.definition),
            qty(s.numerator_usd),
            qty(s.ounces_sold)
        );
        if let Some(share) = s.gold_share {
            let _ = writeln!(
                out,
                "Declared co-product gold allocation: {}.\n",
                qty(share)
            );
        }
        if let Some(next) = &s.superseded_by {
            let _ = writeln!(out, "This definition is superseded by `{}`.\n", text(next));
        }
        out.push_str("| Cost item | Supplied USD | Treatment | Contribution to numerator, USD | Input Claim |\n|---|---:|---|---:|---|\n");
        for c in &s.contributions {
            let units = c.amount.minor.unsigned_abs();
            let amount = format!(
                "{}{}.{:04}",
                if c.amount.minor < 0 { "-" } else { "" },
                units / 10_000,
                units % 10_000
            );
            let _ = writeln!(
                out,
                "| {} | {amount} | {} | {} | `{}` |",
                text(&c.item),
                c.treatment.as_str(),
                qty(c.numerator_usd),
                c.input_claim_id
            );
        }
        let _ = write!(
            out,
            "\nResult Claim: `{}`. Receipt: `{}`.\n\nEvidence: {:?}; knowledge: {:?}; confidentiality: {}.\n\nDeclared complete: **yes**. {}\n\n## Source trace\n\n",
            self.claim().claim_id,
            self.digest(),
            self.claim().evidence,
            self.claim().state,
            self.claim().confidentiality,
            claims::NO_CLAIM
        );
        for input in self.inputs() {
            let _ = writeln!(out, "- `{}` — {:?}", input.claim_id, input.value.value);
            for a in &input.anchors {
                let _ = writeln!(
                    out,
                    "  - {}: {}; SHA-256 `{}`",
                    text(&a.artifact_path),
                    text(&a.locator),
                    Digest32(a.artifact_sha256).to_hex()
                );
            }
        }
        out.push_str("\n## Named unknowns\n\n");
        if self.claim().uu_terms.is_empty() {
            out.push_str("None supplied; this worksheet does not create an unknown-risk budget or authorize a decision.\n");
        }
        for u in &self.claim().uu_terms {
            let _ = writeln!(
                out,
                "- {}: {}; downside exposure {} USD million; trigger: {}.",
                text(&u.scenario_id),
                text(&u.description),
                u.downside_exposure_usd_m,
                text(&u.trigger_indicator)
            );
        }
        out
    }
}
