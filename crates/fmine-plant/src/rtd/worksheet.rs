//! DESIGN: pure source-bound worksheet adapter for the declared passive RTD.
//! One total elemental measurand, dry matter and water share the supplied model.
//! Every opening stock is explicit; optional recovery declarations are separate.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use recovery::{RecoveryComparison, RecoveryDeclaration, TailObservation};

pub const SCHEMA: &str = "fmine.plant-rtd.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = "Conditional passive transport under the submitted equal-tank model, explicit initial stocks and complete feed observations. Any recovery comparison additionally assumes delay then separation, complete tails and recovery of all non-tail metal. Inputs remain Assumed, Internal and uncalibrated. No RTD fitting, inverse deconvolution, reactive inventory, measurement-error inference, empirical calibration, P754 compliance or operating authorization is supplied.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-rtd.v1\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"element\",\"measurand\",\"mass_basis\",\"start_ns\",\"end_ns\",\"opening_ns\",\"tanks\",\"volume_m3\",\"bypass_fraction\",\"dead_fraction\",\"min_flow_m3_per_s\",\"max_flow_m3_per_s\",\"common_conservative_transport\",\"piecewise_constant_feed_per_volume\",\"dry_tolerance_kg\",\"water_tolerance_kg\",\"element_tolerance_kg\",\"max_kernel_steps\"],",
    "\"mass_basis\":\"kg-total/all-phases/full-physical-boundary\",",
    "\"opening_rows\":[\"tank\",\"dry_kg\",\"water_kg\",\"element_kg\"],",
    "\"opening\":\"One row for every tank in 1..tanks and one tank=dead row; zeros must be explicit; opening_ns equals start_ns\",",
    "\"feed_rows\":[\"start_ns\",\"end_ns\",\"slurry_volume_m3\",\"dry_kg\",\"water_kg\",\"element_kg\"],",
    "\"feeds\":\"At most 4096 positive intervals exactly tile the full model window; quantities are integrated totals including bypass\",",
    "\"optional_recovery_row\":[\"delay_then_separation\",\"complete_tail_population\",\"all_non_tail_element_is_recovered\"],",
    "\"tail_rows\":[\"start_ns\",\"end_ns\",\"element_kg\"],",
    "\"recovery\":\"All three declarations must be true and one tail observation must cover each feed interval; zero denominators return null\",",
    "\"values\":\"Quoted trimmed strings of at most 4096 bytes; mass uses plain exact decimals or n/d with a positive denominator, not exponent notation; times use signed integer nanoseconds\",",
    "\"bounds\":\"tanks in 1..64, max_kernel_steps in 1..65536; finite positive volume and allowances, bypass in [0,1], dead fraction in [0,1); interval-mean total flow is checked\",",
    "\"transport\":\"Shared passive response; constant composition per slurry volume within an interval; no unobserved pointwise flow guarantee\",",
    "\"refusal\":\"No admitted Claim, digest or partial trajectory is returned when any declared comparison fails\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; source-byte binding is not independent verification\",",
    "\"examples\":[\"tests/plant/rtd_one_tank.toml\",\"tests/plant/rtd_shift_pulse.toml\"]}"
);

type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    fail(
        RefusalCode::InputMissingOrUnreadable506,
        format!("worksheet line {line}: {}", detail.as_ref()),
    )
}
fn fields<'a>(input: &'a [TomlField], required: &[&str], line: usize) -> Result<Fields<'a>> {
    let mut result = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if !named(value) || value.len() > 4096 {
            return Err(bad(
                field.line,
                "values must be trimmed nonempty text of at most 4096 bytes",
            ));
        }
        result.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(result)
}
fn get<'a>(values: &Fields<'a>, key: &str) -> Result<(&'a str, usize)> {
    values
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}
fn integer(value: (&str, usize)) -> Result<i64> {
    value
        .0
        .parse()
        .map_err(|_| bad(value.1, "expected signed 64-bit integer nanoseconds"))
}
fn count(value: (&str, usize)) -> Result<usize> {
    value
        .0
        .parse()
        .map_err(|_| bad(value.1, "expected an unsigned integer count"))
}
fn boolean(value: (&str, usize)) -> Result<bool> {
    match value.0 {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(bad(value.1, "expected quoted true or false")),
    }
}
fn finite(value: (&str, usize)) -> Result<f64> {
    let parsed = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected a finite number"))?;
    if !parsed.is_finite() {
        return Err(bad(value.1, "expected a finite number"));
    }
    let mantissa = value.0.split(['e', 'E']).next().unwrap_or(value.0);
    if parsed == 0.0 && mantissa.bytes().any(|b| matches!(b, b'1'..=b'9')) {
        return Err(numerical(format!(
            "worksheet line {}: nonzero decimal underflows binary64",
            value.1
        )));
    }
    Ok(parsed)
}
fn quantity(value: (&str, usize)) -> Result<ExactQty> {
    let invalid = || {
        bad(
            value.1,
            "expected a plain exact decimal or n/d with a positive denominator",
        )
    };
    if let Some((n, d)) = value.0.split_once('/') {
        let n = n.parse::<i128>().map_err(|_| invalid())?;
        let d = d.parse::<i128>().map_err(|_| invalid())?;
        if d <= 0 {
            return Err(invalid());
        }
        return ExactQty::new(n, d).map_err(|_| invalid());
    }
    let digits = value.0.strip_prefix('-').unwrap_or(value.0);
    let (whole, fractional) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fractional.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fractional.is_empty())
    {
        return Err(invalid());
    }
    ExactQty::from_decimal_str(value.0).map_err(|_| invalid())
}
fn amounts(values: &Fields<'_>, element: &ElementKey) -> Result<MaterialAmounts> {
    Ok(MaterialAmounts {
        dry_solids_kg: quantity(get(values, "dry_kg")?)?,
        water_kg: quantity(get(values, "water_kg")?)?,
        elements_kg: BTreeMap::from([(element.clone(), quantity(get(values, "element_kg")?)?)]),
    })
}
fn period(values: &Fields<'_>, boundary: &str) -> Result<FlowsheetScope> {
    Ok(FlowsheetScope {
        boundary: boundary.into(),
        interval: ValidTime {
            start_ns: integer(get(values, "start_ns")?)?,
            end_ns: integer(get(values, "end_ns")?)?,
        },
    })
}
fn source_claim<T>(
    value: T,
    tag: &str,
    metric: &str,
    additivity: AdditivityClass,
    source: &SourceAnchor,
    locator: String,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::internal(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            locator,
        )],
    )
}

#[derive(Debug, Clone, PartialEq)]
struct RecoveryReport {
    declaration: Claim<RecoveryDeclaration>,
    observations: Vec<Claim<TailObservation>>,
    claim: Claim<RecoveryComparison>,
    digest: String,
}

/// Complete immutable transport and, when requested, complete recovery comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct WorksheetResult {
    submitted: String,
    element: ElementKey,
    transport: RtdReceipt,
    recovery: Option<RecoveryReport>,
}

/// Parse exact source bytes and evaluate all requested outputs atomically.
pub fn evaluate(text: &str, source: &SourceAnchor) -> Result<WorksheetResult> {
    if text.len() as u64 > MAX_BYTES
        || !named(&source.artifact_path)
        || !named(&source.locator)
        || source.artifact_path.len() > 4096
        || source.locator.len() > 4096
        || sha256(text.as_bytes()).0 != source.artifact_sha256
    {
        return Err(bad(
            1,
            "bounded worksheet bytes must match a named source anchor and SHA-256",
        ));
    }
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if document.rows.len() > MAX_INTERVALS * 2 + 66 {
        return Err(bad(1, "too many opening, feed, recovery or tail rows"));
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "boundary",
            "element",
            "measurand",
            "mass_basis",
            "start_ns",
            "end_ns",
            "opening_ns",
            "tanks",
            "volume_m3",
            "bypass_fraction",
            "dead_fraction",
            "min_flow_m3_per_s",
            "max_flow_m3_per_s",
            "common_conservative_transport",
            "piecewise_constant_feed_per_volume",
            "dry_tolerance_kg",
            "water_tolerance_kg",
            "element_tolerance_kg",
            "max_kernel_steps",
        ],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "mass_basis")?.0 != "kg-total/all-phases/full-physical-boundary" {
        return Err(bad(
            1,
            "mass_basis must be kg-total/all-phases/full-physical-boundary",
        ));
    }
    let element = ElementKey {
        element: get(&top, "element")?.0.into(),
        measurand: get(&top, "measurand")?.0.into(),
    };
    let scope = period(&top, get(&top, "boundary")?.0)?;
    let tanks = count(get(&top, "tanks")?)?;
    if !(1..=64).contains(&tanks) {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "tanks must be in 1..64",
        ));
    }
    let model = RtdModel {
        scope: scope.clone(),
        elements: BTreeSet::from([element.clone()]),
        tanks,
        volume_m3: CubicMetres::new(finite(get(&top, "volume_m3")?)?),
        bypass_fraction: finite(get(&top, "bypass_fraction")?)?,
        dead_fraction: finite(get(&top, "dead_fraction")?)?,
        total_flow_domain_m3_per_s: [
            CubicMetresPerSecond::new(finite(get(&top, "min_flow_m3_per_s")?)?),
            CubicMetresPerSecond::new(finite(get(&top, "max_flow_m3_per_s")?)?),
        ],
        common_conservative_transport: boolean(get(&top, "common_conservative_transport")?)?,
        piecewise_constant_feed_per_volume: boolean(get(
            &top,
            "piecewise_constant_feed_per_volume",
        )?)?,
        closure_tolerances_kg: BTreeMap::from([
            (
                MaterialChannel::DrySolids,
                Kilograms::new(finite(get(&top, "dry_tolerance_kg")?)?),
            ),
            (
                MaterialChannel::Water,
                Kilograms::new(finite(get(&top, "water_tolerance_kg")?)?),
            ),
            (
                MaterialChannel::Element(element.clone()),
                Kilograms::new(finite(get(&top, "element_tolerance_kg")?)?),
            ),
        ]),
        max_kernel_steps: count(get(&top, "max_kernel_steps")?)?,
    };
    let tag = model.canonical_tag();
    let model = source_claim(
        model,
        &tag,
        MODEL_METRIC,
        AdditivityClass::NonAdditive,
        source,
        "all top-level model, support, domain, allowance and transport declarations".into(),
    );
    let mut opening_rows = BTreeMap::new();
    let mut dead = None;
    let mut feeds = Vec::new();
    let mut tails = Vec::new();
    let mut recovery_flags = None;
    for row in &document.rows {
        match row.table.as_str() {
            "opening" => {
                let values = fields(
                    &row.fields,
                    &["tank", "dry_kg", "water_kg", "element_kg"],
                    row.line,
                )?;
                let stock = amounts(&values, &element)?;
                let tank = get(&values, "tank")?;
                if tank.0 == "dead" {
                    if dead.replace(stock).is_some() {
                        return Err(bad(row.line, "duplicate dead opening stock"));
                    }
                } else {
                    let index = count(tank)?;
                    if !(1..=tanks).contains(&index) {
                        return Err(bad(row.line, "opening tank must be in 1..tanks or dead"));
                    }
                    if opening_rows.insert(index, stock).is_some() {
                        return Err(bad(row.line, "duplicate tank opening stock"));
                    }
                }
            }
            "feed" => {
                let values = fields(
                    &row.fields,
                    &[
                        "start_ns",
                        "end_ns",
                        "slurry_volume_m3",
                        "dry_kg",
                        "water_kg",
                        "element_kg",
                    ],
                    row.line,
                )?;
                let feed = RtdFeed {
                    scope: period(&values, &scope.boundary)?,
                    slurry_volume_m3: CubicMetres::new(finite(get(&values, "slurry_volume_m3")?)?),
                    amounts: amounts(&values, &element)?,
                };
                let tag = feed.canonical_tag();
                feeds.push(source_claim(
                    feed,
                    &tag,
                    FEED_METRIC,
                    AdditivityClass::ExtensiveAdditive,
                    source,
                    format!(
                        "feed row at line {}; all integrated totals and support",
                        row.line
                    ),
                ));
            }
            "recovery" => {
                let values = fields(
                    &row.fields,
                    &[
                        "delay_then_separation",
                        "complete_tail_population",
                        "all_non_tail_element_is_recovered",
                    ],
                    row.line,
                )?;
                let flags = (
                    boolean(get(&values, "delay_then_separation")?)?,
                    boolean(get(&values, "complete_tail_population")?)?,
                    boolean(get(&values, "all_non_tail_element_is_recovered")?)?,
                    row.line,
                );
                if recovery_flags.replace(flags).is_some() {
                    return Err(bad(row.line, "at most one recovery declaration is allowed"));
                }
            }
            "tail" => {
                let values = fields(&row.fields, &["start_ns", "end_ns", "element_kg"], row.line)?;
                let observation = TailObservation {
                    scope: period(&values, &scope.boundary)?,
                    element: element.clone(),
                    tail_element_kg: quantity(get(&values, "element_kg")?)?,
                };
                let tag = observation.canonical_tag();
                tails.push(source_claim(
                    observation,
                    &tag,
                    recovery::TAIL_METRIC,
                    AdditivityClass::ExtensiveAdditive,
                    source,
                    format!(
                        "tail row at line {}; exact elemental mass and support",
                        row.line
                    ),
                ));
            }
            _ => return Err(bad(row.line, format!("unsupported table {}", row.table))),
        }
    }
    if opening_rows.len() != tanks {
        return Err(bad(
            1,
            "every tank opening stock is required, including explicit zero totals",
        ));
    }
    let dead = dead.ok_or_else(|| {
        bad(
            1,
            "an explicit dead opening stock is required, including zeros",
        )
    })?;
    let opening = RtdOpening {
        boundary: scope.boundary.clone(),
        at_ns: integer(get(&top, "opening_ns")?)?,
        tanks: opening_rows.into_values().collect(),
        dead,
    };
    let tag = opening.canonical_tag();
    let opening = source_claim(
        opening,
        &tag,
        OPENING_METRIC,
        AdditivityClass::ExtensiveAdditive,
        source,
        format!(
            "all opening rows and opening_ns at line {}",
            get(&top, "opening_ns")?.1
        ),
    );
    if recovery_flags.is_none() && !tails.is_empty() {
        return Err(bad(
            1,
            "tail observations require an explicit recovery declaration",
        ));
    }
    let transport = propagate(&model, &opening, &feeds, || false)?;
    let recovery = if let Some((delay, complete, recovered, line)) = recovery_flags {
        let declaration = RecoveryDeclaration {
            transport_claim: transport.claim().claim_id,
            element: element.clone(),
            delay_then_separation: delay,
            complete_tail_population: complete,
            all_non_tail_element_is_recovered: recovered,
        };
        let tag = declaration.canonical_tag();
        let declaration = source_claim(
            declaration,
            &tag,
            recovery::DECLARATION_METRIC,
            AdditivityClass::NonAdditive,
            source,
            format!("recovery assumptions at line {line}"),
        );
        let receipt = recovery::compare_recovery(&transport, &declaration, &tails)?;
        Some(RecoveryReport {
            declaration: receipt.declaration().clone(),
            observations: receipt.observations().to_vec(),
            claim: receipt.claim().clone(),
            digest: receipt.digest().into(),
        })
    } else {
        None
    };
    Ok(WorksheetResult {
        submitted: text.into(),
        element,
        transport,
        recovery,
    })
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn ratio(value: Option<f64>) -> String {
    value.map_or_else(|| "null".into(), number)
}
fn claim_json<T>(claim: &Claim<T>) -> String {
    let sources = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
                quoted(&a.artifact_path),
                quoted(&Digest32(a.artifact_sha256).to_hex()),
                quoted(&a.locator),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = claim
        .parents
        .iter()
        .map(|id| quoted(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"metric\":{},\"evidence\":{},\"state\":{},\"confidentiality\":\"Internal\",\"calibration\":\"uncalibrated\",\"parents\":[{parents}],\"sources\":[{sources}]}}",
        quoted(&claim.claim_id.to_string()),
        quoted(&claim.metric_key),
        quoted(&format!("{:?}", claim.evidence)),
        quoted(&format!("{:?}", claim.state)),
    )
}
fn masses(values: &ChannelMasses) -> String {
    let fields = values
        .iter()
        .map(|(channel, quantity)| {
            let name = match channel {
                MaterialChannel::DrySolids => "dry_kg",
                MaterialChannel::Water => "water_kg",
                MaterialChannel::Element(_) => "element_kg",
            };
            format!("\"{name}\":{}", number(quantity.value()))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}
fn interval_json(interval: &RtdInterval) -> String {
    format!(
        concat!(
            "{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"feed_source\":{},\"active_tank_turnovers\":{},",
            "\"opening_inventory_kg\":{},\"feed_kg\":{},\"bypass_kg\":{},\"tank_outflow_kg\":{},",
            "\"total_outflow_kg\":{},\"closing_inventory_kg\":{},\"balance_residual_kg\":{}}}"
        ),
        interval.scope.interval.start_ns,
        interval.scope.interval.end_ns,
        quoted(&interval.feed_source.to_string()),
        number(interval.active_tank_turnovers),
        masses(&interval.opening_inventory_kg),
        masses(&interval.feed_kg),
        masses(&interval.bypass_kg),
        masses(&interval.tank_outflow_kg),
        masses(&interval.total_outflow_kg),
        masses(&interval.closing_inventory_kg),
        masses(&interval.balance_residual_kg),
    )
}
fn recovery_json(report: &RecoveryReport) -> String {
    let observations = report
        .observations
        .iter()
        .map(claim_json)
        .collect::<Vec<_>>()
        .join(",");
    let intervals = report.claim.value.intervals.iter().map(|value| {
        format!(
            concat!(
                "{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"tail_source\":{},\"tail_element_kg\":{},",
                "\"same_interval_feed_element_kg\":{},\"aligned_feed_equivalent_kg\":{},",
                "\"same_interval_recovery\":{},\"lag_aligned_recovery\":{}}}"
            ),
            value.interval.start_ns, value.interval.end_ns, quoted(&value.tail_source.to_string()),
            number(value.tail_element_kg.value()), number(value.same_interval_feed_element_kg.value()),
            number(value.aligned_feed_equivalent_kg.value()), ratio(value.same_interval_recovery),
            ratio(value.lag_aligned_recovery),
        )
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"declaration\":{},\"observations\":[{observations}],\"admitted_claim\":{},\"receipt_digest\":{},\"result\":{{\"intervals\":[{intervals}]}}}}",
        claim_json(&report.declaration),
        claim_json(&report.claim),
        quoted(&report.digest),
    )
}

impl WorksheetResult {
    pub fn transport(&self) -> &RtdReceipt {
        &self.transport
    }
    pub fn to_json(&self) -> String {
        let result = self.transport.summary();
        let feeds = self
            .transport
            .feeds()
            .iter()
            .map(claim_json)
            .collect::<Vec<_>>()
            .join(",");
        let intervals = result
            .intervals
            .iter()
            .map(interval_json)
            .collect::<Vec<_>>()
            .join(",");
        let tanks = result
            .final_tanks_kg
            .iter()
            .map(masses)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                "{{\"schema\":\"fmine.plant-rtd-result.v1\",\"submitted_worksheet\":{},",
                "\"boundary\":{},\"element\":{},\"measurand\":{},\"model\":{},\"opening\":{},\"feeds\":[{}],",
                "\"admitted_claim\":{},\"receipt_digest\":{},\"result\":{{\"intervals\":[{}],",
                "\"final_tanks_kg\":[{}],\"dead_inventory_kg\":{},\"whole_window_residual_kg\":{},\"kernel_steps\":{}}},",
                "\"recovery\":{}}}"
            ),
            quoted(&self.submitted),
            quoted(&result.scope.boundary),
            quoted(&self.element.element),
            quoted(&self.element.measurand),
            claim_json(self.transport.model()),
            claim_json(self.transport.opening()),
            feeds,
            claim_json(self.transport.claim()),
            quoted(self.transport.digest()),
            intervals,
            tanks,
            masses(&result.dead_inventory_kg),
            masses(&result.whole_window_residual_kg),
            result.kernel_steps,
            self.recovery
                .as_ref()
                .map_or_else(|| "null".into(), recovery_json),
        )
    }
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Declared plant residence-time transport\n\n{NO_CLAIM}\n\n\
             All quantities below are kilograms; elemental mass overlaps dry matter and water.\n\n\
             | Interval (ns) | Channel | Opening | Feed | Bypass | Tank outflow | Total outflow | Closing | Residual |\n\
             |---|---|---:|---:|---:|---:|---:|---:|---:|\n"
        );
        for interval in &self.transport.summary().intervals {
            for (channel, label) in [
                (MaterialChannel::DrySolids, "dry matter"),
                (MaterialChannel::Water, "water"),
                (
                    MaterialChannel::Element(self.element.clone()),
                    "total element",
                ),
            ] {
                out.push_str(&format!(
                    "| {}..{} | {label} | {} | {} | {} | {} | {} | {} | {} |\n",
                    interval.scope.interval.start_ns,
                    interval.scope.interval.end_ns,
                    number(interval.opening_inventory_kg[&channel].value()),
                    number(interval.feed_kg[&channel].value()),
                    number(interval.bypass_kg[&channel].value()),
                    number(interval.tank_outflow_kg[&channel].value()),
                    number(interval.total_outflow_kg[&channel].value()),
                    number(interval.closing_inventory_kg[&channel].value()),
                    number(interval.balance_residual_kg[&channel].value()),
                ));
            }
        }
        out.push_str("\n## Whole-window conservation residuals\n\n");
        for (channel, quantity) in &self.transport.summary().whole_window_residual_kg {
            let label = match channel {
                MaterialChannel::DrySolids => "Dry matter",
                MaterialChannel::Water => "Water",
                MaterialChannel::Element(_) => "Total element",
            };
            out.push_str(&format!("{label}: {} kg.\n\n", number(quantity.value())));
        }
        if let Some(report) = &self.recovery {
            out.push_str(
                "## Conditional recovery comparison\n\nFractions, under the declared delay-then-separation hypothesis. Undefined means a zero denominator.\n\n\
                 | Interval (ns) | Same-period feed kg | Aligned feed-equivalent kg | Tail kg | Same-period recovery | Lag-aligned recovery |\n\
                 |---|---:|---:|---:|---:|---:|\n",
            );
            for value in &report.claim.value.intervals {
                out.push_str(&format!(
                    "| {}..{} | {} | {} | {} | {} | {} |\n",
                    value.interval.start_ns,
                    value.interval.end_ns,
                    number(value.same_interval_feed_element_kg.value()),
                    number(value.aligned_feed_equivalent_kg.value()),
                    number(value.tail_element_kg.value()),
                    value
                        .same_interval_recovery
                        .map_or_else(|| "undefined".into(), number),
                    value
                        .lag_aligned_recovery
                        .map_or_else(|| "undefined".into(), number),
                ));
            }
        }
        out.push_str(&format!(
            "\nTransport receipt: {}.\n",
            self.transport.digest()
        ));
        out
    }
}
