//! Source-bound PDF sums, cost rates and reserve roll-forwards (BD-340.S5, BD-331.S7).
//!
//! The caller verifies registered document bytes and reader agreement. This pure adapter rechecks the
//! admitted rows against those declarations, preserves each filing vintage, and delegates every arithmetic
//! verdict to the checked exact solver. It never creates a reserve classification or chooses a corrected value.
//! Determinism: DeterministicPerIsa (table checks are exact; optional cutoffs inherit a float solver cross-check).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::hash::sha256;
use fmine_numerics::exact_feas::{self as xf, Rat};
use fmine_registry::metrics::{MetricDefinition, MetricFamily, MetricRegistry};
use fmine_registry::sources::{DocumentRow, is_date};
use fmine_registry::table_identities::{
    IdentityKind, TableIdentity, TableIdentityRegistry, TermRole,
};
use fmine_registry::transcriptions::{Accepted, Token, parse_token};

pub mod quarter;

pub const SCHEMA: &str = "fmine.pdf-table-check.v1";
pub const NO_CLAIM: &str = "Checks the declared identities of registered issuer assertions under stated rounding and membership assumptions. It does not verify the truth of the issuer's figures, establish complete reserve coverage, select a true vintage, classify a reserve, or supply a corrected published value. Document-byte authentication and independent reader agreement are caller obligations.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCell {
    pub cell: xf::Cell,
    pub source: Accepted,
    pub document: DocumentRow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityRefusal {
    pub identity: String,
    pub doc: String,
    pub reason: String,
    /// Available candidate readings, retained even when their labels or multiplicity prevent binding.
    pub evidence: Vec<Accepted>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCheck {
    pub component: xf::Component,
    pub rules: xf::Rules,
    pub certificate_digest: Option<String>,
    pub blame: Option<xf::BlameOutcome>,
    /// Ranges satisfying every declared identity and printed box in a feasible component. These are
    /// mathematical possibilities, not issuer assertions; correction-set ranges appear in `blame` instead.
    pub implied: Vec<(String, Result<xf::Interval, String>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableReport {
    pub issuer: String,
    pub definitions: Vec<TableIdentity>,
    /// Exact registered definitions selected by replacement identities, retained in the report digest.
    pub metric_definitions: Vec<MetricDefinition>,
    pub cells: Vec<SourceCell>,
    pub components: Vec<ComponentCheck>,
    pub refused: Vec<IdentityRefusal>,
    pub cutoffs: crate::cutoff::source::SourceReport,
}

pub(crate) fn convention(name: &str) -> xf::Convention {
    match name {
        "half_up" => xf::Convention::HalfUp,
        "half_down" => xf::Convention::HalfDown,
        "half_even" => xf::Convention::HalfEven,
        "truncate" => xf::Convention::Truncate,
        // Registry validation, performed at this boundary, admits only these five names.
        _ => xf::Convention::Closed,
    }
}

fn construction(name: &str) -> xf::Construction {
    if name == "component_sum" {
        xf::Construction::ComponentSum
    } else {
        xf::Construction::RoundedOnce
    }
}

fn reading_key(a: &Accepted) -> (&str, &str, &str, &str, &str, &str) {
    (
        &a.quantity,
        &a.token,
        &a.unit,
        &a.basis,
        &a.period,
        &a.doc_sha256,
    )
}

pub(crate) fn canonical_readings(readings: &mut [Accepted]) {
    let value_key = |a: &Accepted| match a.value {
        Token::Point(d) => (0_u8, d.mantissa, d.scale, 0, 0),
        Token::Range(lo, hi) => (1, lo.mantissa, lo.scale, hi.mantissa, hi.scale),
        Token::AtMost(d) => (2, d.mantissa, d.scale, 0, 0),
        Token::AtLeast(d) => (3, d.mantissa, d.scale, 0, 0),
    };
    for reading in readings.iter_mut() {
        reading.readers.sort();
    }
    readings.sort_by(|a, b| {
        (&a.locator, reading_key(a), value_key(a), &a.readers).cmp(&(
            &b.locator,
            reading_key(b),
            value_key(b),
            &b.readers,
        ))
    });
}

fn selected_readings(identity: &TableIdentity, readings: &[Accepted]) -> Vec<Accepted> {
    let mut selected: Vec<_> = readings
        .iter()
        .filter(|a| {
            a.locator.doc == identity.doc
                && (a.quantity == identity.total
                    || identity.terms.iter().any(|t| t.quantity == a.quantity))
        })
        .cloned()
        .collect();
    canonical_readings(&mut selected);
    selected
}

fn source_cell(
    definition: &TableIdentity,
    quantity: &str,
    period: &str,
    unit: &str,
    readings: &[Accepted],
    document: &DocumentRow,
) -> Result<SourceCell, String> {
    bind_reading(
        &definition.doc,
        quantity,
        period,
        unit,
        &definition.basis,
        readings,
        document,
    )
}

/// Shared source boundary for arithmetic and typed issuer reserve assertions. Bytes are checked by callers.
pub(crate) fn bind_reading(
    doc: &str,
    quantity: &str,
    period: &str,
    unit: &str,
    basis: &str,
    readings: &[Accepted],
    document: &DocumentRow,
) -> Result<SourceCell, String> {
    let matches: Vec<_> = readings
        .iter()
        .filter(|a| a.locator.doc == doc && a.quantity == quantity)
        .collect();
    let a = match matches.as_slice() {
        [a] => *a,
        [] => return Err(format!("no agreed reading for quantity {quantity}")),
        _ => {
            return Err(format!(
                "quantity {quantity} has multiple candidate readings"
            ));
        }
    };
    let refuse = |why: &str| format!("{}: {why}", a.locator);
    if readings
        .iter()
        .filter(|other| other.locator == a.locator)
        .count()
        != 1
    {
        return Err(refuse("locator appears more than once"));
    }
    // The solver's cell id is the displayed citation. Labels may themselves contain ':' or '/', so two
    // distinct structured locators can render identically; neither may silently replace the other in a map.
    let rendered = a.locator.to_string();
    if readings
        .iter()
        .filter(|other| other.locator.to_string() == rendered)
        .count()
        != 1
    {
        return Err(refuse(
            "ambiguous rendered locator from distinct source labels",
        ));
    }
    if a.unit != unit || a.basis != basis || a.period != period {
        return Err(refuse(
            "unit, basis or period differs from the explicit declaration",
        ));
    }
    if a.doc_sha256 != document.sha256
        || document.sha256.len() != 64
        || !document
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !is_date(&document.published_at)
        || document.kind.trim().is_empty()
        || document.published_at_basis.trim().is_empty()
        || a.locator.page == 0
        || a.locator.page > document.pages
        || [&a.locator.table, &a.locator.row, &a.locator.col]
            .iter()
            .any(|s| s.trim().is_empty())
    {
        return Err(refuse(
            "source locator, publication metadata or registered digest is invalid",
        ));
    }
    let readers: BTreeSet<_> = a.readers.iter().collect();
    if readers.len() < 2
        || readers.len() != a.readers.len()
        || readers.iter().any(|r| {
            r.is_empty()
                || !r.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-')
                })
        })
    {
        return Err(refuse(
            "at least two distinct reader role identities are required",
        ));
    }
    if parse_token(&a.token).as_ref() != Ok(&a.value) {
        return Err(refuse("printed token and exact admitted value disagree"));
    }
    let Token::Point(decimal) = a.value else {
        return Err(refuse(
            "a table cell must be a printed point, not a guidance range",
        ));
    };
    let den = 10_i128
        .checked_pow(decimal.scale)
        .ok_or_else(|| refuse("printed precision overflow"))?;
    let printed = Rat::new(decimal.mantissa, den).map_err(|_| refuse("printed value overflow"))?;
    let step = Rat::new(1, den).map_err(|_| refuse("printed precision overflow"))?;
    let mut source = a.clone();
    source.readers.sort();
    Ok(SourceCell {
        cell: xf::Cell {
            locator: a.locator.to_string(),
            sha256: a.doc_sha256.clone(),
            token: a.token.clone(),
            printed,
            step,
            scope: xf::Scope::Pdf,
        },
        source,
        document: document.clone(),
    })
}

fn bind(
    definition: &TableIdentity,
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> Result<(xf::Identity, Vec<SourceCell>), String> {
    let cost_rate = definition.kind.cost_rate();
    let document = documents
        .iter()
        .find(|d| d.file == definition.doc)
        .ok_or_else(|| "document is not registered".to_string())?;
    let total = source_cell(
        definition,
        &definition.total,
        &definition.total_period,
        &definition.unit,
        readings,
        document,
    )?;
    let mut sources = vec![total];
    let mut terms = Vec::new();
    for term in &definition.terms {
        let unit = match (&definition.replacement, cost_rate, term.role) {
            (Some(r), _, TermRole::Numerator) => r.numerator_unit.as_str(),
            (Some(r), _, TermRole::Denominator) => r.denominator_unit.as_str(),
            (_, Some(r), TermRole::Numerator) => r.numerator_unit,
            (_, Some(r), TermRole::Denominator) => r.denominator_unit,
            _ => &definition.unit,
        };
        let source = source_cell(
            definition,
            &term.quantity,
            &term.period,
            unit,
            readings,
            document,
        )?;
        terms.push((term, source.cell.locator.clone()));
        sources.push(source);
    }
    let total = &sources[0].cell.locator;
    let identity = match definition.kind {
        IdentityKind::Sum => xf::Identity {
            id: definition.id.clone(),
            family: definition.family.clone(),
            closure: xf::Closure::Definitional,
            kind: xf::IdentityKind::Sum {
                total: total.clone(),
                lines: terms.iter().map(|(_, loc)| loc.clone()).collect(),
                signs: terms.iter().map(|(term, _)| term.sign).collect(),
            },
        },
        IdentityKind::RollForward => {
            let opening = terms
                .iter()
                .find(|(t, _)| t.role == TermRole::Opening)
                .ok_or_else(|| "roll-forward has no opening".to_string())?;
            let moves: Vec<_> = terms
                .iter()
                .filter(|(t, _)| t.role == TermRole::Move)
                .map(|(t, loc)| (loc.as_str(), t.sign))
                .collect();
            xf::Identity::roll_forward(
                &definition.id,
                &definition.family,
                &opening.1,
                &moves,
                total,
            )
        }
        IdentityKind::ReserveReplacement
        | IdentityKind::GoldCostPerSoldOunce
        | IdentityKind::CopperCostPerSoldPound => {
            let factor = match cost_rate {
                Some(rate) => rate.factor,
                None => {
                    definition
                        .replacement
                        .as_ref()
                        .ok_or("missing replacement metadata")?
                        .factor
                }
            };
            let numerator = terms
                .iter()
                .find(|(t, _)| t.role == TermRole::Numerator)
                .ok_or("missing numerator")?;
            let denominator = terms
                .iter()
                .find(|(t, _)| t.role == TermRole::Denominator)
                .ok_or("missing denominator")?;
            let cell = sources
                .iter()
                .find(|s| s.cell.locator == denominator.1)
                .ok_or("missing denominator source")?;
            let half = cell
                .cell
                .step
                .checked_div(Rat::from_integer(2))
                .map_err(|e| e.to_string())?;
            let bounds = convention(&definition.convention)
                .interval(cell.cell.printed, half)
                .map_err(|e| format!("denominator rounding interval unavailable: {e:?}"))?;
            let minimum = if denominator.0.sign == 1 {
                bounds.lo
            } else {
                bounds.hi.checked_neg().map_err(|e| e.to_string())?
            };
            if minimum.signum() <= 0 {
                return Err(if cost_rate.is_some() {
                    "sold-metal denominator rounding interval must be strictly positive and bounded away from zero"
                } else {
                    "signed depletion denominator rounding interval must be strictly positive and bounded away from zero"
                }.into());
            }
            xf::Identity {
                id: definition.id.clone(),
                family: definition.family.clone(),
                closure: xf::Closure::Definitional,
                kind: xf::IdentityKind::Ratio {
                    rate: total.clone(),
                    num: numerator.1.clone(),
                    den: denominator.1.clone(),
                    factor: Rat::from_integer(
                        i128::from(factor)
                            * i128::from(numerator.0.sign)
                            * i128::from(denominator.0.sign),
                    ),
                },
            }
        }
    };
    Ok((identity, sources))
}

/// Checks every complete declared identity, retaining refusals in the declared denominator. Shared cells
/// within a document are solved jointly. Distinct documents remain distinct vintages; there is no precedence
/// rule or implicit cross-vintage equality. Caller-authenticated bytes are required for a source-backed use.
#[must_use]
pub fn assess(
    registry: &TableIdentityRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> TableReport {
    assess_inner(registry, readings, documents, None)
}

/// Also authenticates each declared replacement definition against the supplied metric registry.
/// The caller still verifies document bytes and reader agreement. No GEO conversion is performed.
#[must_use]
pub fn assess_with_metrics(
    registry: &TableIdentityRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
    metrics: &MetricRegistry,
) -> TableReport {
    assess_inner(registry, readings, documents, Some(metrics))
}

fn assess_inner(
    registry: &TableIdentityRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
    metrics: Option<&MetricRegistry>,
) -> TableReport {
    let mut definitions = registry.identities.clone();
    definitions.sort_by(|a, b| a.id.cmp(&b.id));
    for definition in &mut definitions {
        definition.terms.sort_by(|a, b| a.quantity.cmp(&b.quantity));
    }
    let mut report = TableReport {
        issuer: registry.issuer.clone(),
        definitions,
        metric_definitions: Vec::new(),
        cells: Vec::new(),
        components: Vec::new(),
        refused: Vec::new(),
        cutoffs: crate::cutoff::source::assess(&registry.cutoffs, readings, documents),
    };
    if let Err(error) = registry.validate(documents) {
        report.cutoffs.refuse_all(&error.to_string());
        report.refused.push(IdentityRefusal {
            identity: "registry".into(),
            doc: String::new(),
            reason: error.to_string(),
            evidence: Vec::new(),
        });
        return report;
    }
    let mut groups: BTreeMap<String, Vec<(xf::Identity, Vec<SourceCell>)>> = BTreeMap::new();
    for definition in &report.definitions {
        let bound = (|| {
            if let Some(replacement) = &definition.replacement {
                let selected: Vec<_> = metrics
                    .into_iter()
                    .flat_map(|m| &m.definitions)
                    .filter(|d| d.id == replacement.definition)
                    .collect();
                let [metric] = selected.as_slice() else {
                    return Err(format!(
                        "replacement requires exactly one registered metric definition {}",
                        replacement.definition
                    ));
                };
                if metric.family != MetricFamily::ReserveReplacement
                    || metric.unit != "percent"
                    || metric.formula.trim().is_empty()
                    || metric.source.trim().is_empty()
                    || metric.evidence.trim().is_empty()
                {
                    return Err("replacement definition must be a cited reserve_replacement percentage definition".into());
                }
                if !report.metric_definitions.iter().any(|d| d.id == metric.id) {
                    report.metric_definitions.push((*metric).clone());
                }
            }
            bind(definition, readings, documents)
        })();
        match bound {
            Ok(bound) => groups
                .entry(definition.doc.clone())
                .or_default()
                .push(bound),
            Err(reason) => report.refused.push(IdentityRefusal {
                identity: definition.id.clone(),
                doc: definition.doc.clone(),
                reason,
                evidence: selected_readings(definition, readings),
            }),
        }
    }
    report.metric_definitions.sort_by(|a, b| a.id.cmp(&b.id));
    for (doc, bound) in groups {
        let definitions: Vec<_> = report.definitions.iter().filter(|d| d.doc == doc).collect();
        let Some(first) = definitions.first() else {
            continue;
        };
        let mut rules = xf::Rules::new(convention(&first.convention));
        for definition in &definitions {
            rules = rules.with(&definition.family, construction(&definition.construction));
        }
        let identities: Vec<_> = bound.iter().map(|(id, _)| id.clone()).collect();
        let sources: BTreeMap<_, _> = bound
            .into_iter()
            .flat_map(|(_, sources)| sources)
            .map(|s| (s.cell.locator.clone(), s))
            .collect();
        let cells: Vec<_> = sources.values().map(|s| s.cell.clone()).collect();
        report.cells.extend(sources.into_values());
        let components = match xf::solve(&cells, &identities, &rules) {
            Ok(components) => components,
            Err(error) => {
                for identity in identities {
                    report.refused.push(IdentityRefusal {
                        identity: identity.id,
                        doc: doc.clone(),
                        reason: error.to_string(),
                        evidence: Vec::new(),
                    });
                }
                continue;
            }
        };
        for mut component in components {
            let component_cells: Vec<_> = cells
                .iter()
                .filter(|c| component.cells.contains(&c.locator))
                .cloned()
                .collect();
            let component_ids: Vec<_> = identities
                .iter()
                .filter(|i| component.identities.contains(&i.id))
                .cloned()
                .collect();
            let checked = match &component.outcome {
                xf::Outcome::Feasible { witness } => {
                    xf::check_witness(&component_cells, &component_ids, &rules, witness)
                }
                xf::Outcome::Infeasible { certificate } => {
                    xf::check_certificate(&component_cells, &component_ids, &rules, certificate)
                        .map(|_| ())
                }
                _ => Ok(()),
            };
            if let Err(error) = checked {
                component.outcome = xf::Outcome::Indeterminate {
                    reason: format!("certificate check failed: {}", error.0),
                };
            }
            let certificate_digest = if let xf::Outcome::Infeasible { certificate } =
                &component.outcome
            {
                match xf::certificate_digest(&component_cells, &component_ids, &rules, certificate)
                {
                    Ok(digest) => Some(digest),
                    Err(error) => {
                        component.outcome = xf::Outcome::Indeterminate {
                            reason: error.to_string(),
                        };
                        None
                    }
                }
            } else {
                None
            };
            let blame = if matches!(component.outcome, xf::Outcome::Infeasible { .. }) {
                Some(match xf::blame(&component_cells, &component_ids, &rules) {
                    Ok(mut blamed) if blamed.len() == 1 => blamed.remove(0).outcome,
                    Ok(_) => xf::BlameOutcome::Indeterminate {
                        reason: "no unique component blame result".into(),
                    },
                    Err(error) => xf::BlameOutcome::Indeterminate {
                        reason: error.to_string(),
                    },
                })
            } else {
                None
            };
            let implied = if matches!(component.outcome, xf::Outcome::Feasible { .. }) {
                component
                    .cells
                    .iter()
                    .map(|loc| {
                        (
                            loc.clone(),
                            xf::implied_interval(
                                &component_cells,
                                &component_ids,
                                &rules,
                                loc,
                                false,
                            )
                            .map_err(|e| format!("{e:?}")),
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            report.components.push(ComponentCheck {
                component,
                rules: rules.clone(),
                certificate_digest,
                blame,
                implied,
            });
        }
    }
    report
        .cells
        .sort_by(|a, b| a.cell.locator.cmp(&b.cell.locator));
    report
        .components
        .sort_by(|a, b| a.component.identities.cmp(&b.component.identities));
    report
        .refused
        .sort_by(|a, b| (&a.identity, &a.doc, &a.reason).cmp(&(&b.identity, &b.doc, &b.reason)));
    report
}

pub(crate) fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn strings(values: &[String]) -> String {
    format!(
        "[{}]",
        values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
    )
}

pub(crate) fn rat(value: Rat) -> String {
    format!(
        "{{\"num\":\"{}\",\"den\":\"{}\"}}",
        value.num(),
        value.den()
    )
}

pub(crate) fn interval(value: &xf::Interval) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
        rat(value.lo),
        rat(value.hi),
        value.lo_strict,
        value.hi_strict
    )
}

pub(crate) fn accepted_json(a: &Accepted) -> String {
    let decimal = |d: fmine_registry::transcriptions::Decimal| {
        format!("{{\"mantissa\":\"{}\",\"scale\":{}}}", d.mantissa, d.scale)
    };
    let value = match a.value {
        Token::Point(d) => format!("{{\"kind\":\"point\",\"value\":{}}}", decimal(d)),
        Token::Range(lo, hi) => format!(
            "{{\"kind\":\"range\",\"lo\":{},\"hi\":{}}}",
            decimal(lo),
            decimal(hi)
        ),
        Token::AtMost(d) => format!("{{\"kind\":\"at_most\",\"upper\":{}}}", decimal(d)),
        Token::AtLeast(d) => format!("{{\"kind\":\"at_least\",\"lower\":{}}}", decimal(d)),
    };
    format!(
        "{{\"locator\":{},\"doc\":{},\"page\":{},\"table\":{},\"row\":{},\"col\":{},\"sha256\":{},\"token\":{},\"admitted_value\":{value},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"readers\":{}}}",
        js(&a.locator.to_string()),
        js(&a.locator.doc),
        a.locator.page,
        js(&a.locator.table),
        js(&a.locator.row),
        js(&a.locator.col),
        js(&a.doc_sha256),
        js(&a.token),
        js(&a.quantity),
        js(&a.unit),
        js(&a.basis),
        js(&a.period),
        strings(&a.readers)
    )
}

pub(crate) fn definition_json(d: &TableIdentity) -> String {
    let replacement = d.replacement.as_ref().map_or_else(|| "null".into(), |r| format!(
        "{{\"definition\":{},\"numerator_unit\":{},\"denominator_unit\":{},\"factor\":{},\"price_deck\":{}}}",
        js(&r.definition), js(&r.numerator_unit), js(&r.denominator_unit), r.factor, js(&r.price_deck)
    ));
    let cost_rate = d.kind.cost_rate().map_or_else(String::new, |r| format!(
        ",\"cost_rate\":{{\"numerator_unit\":{},\"denominator_unit\":{},\"unit\":{},\"factor\":{},\"formula\":\"factor*cost/sold\"}}",
        js(r.numerator_unit), js(r.denominator_unit), js(r.unit), r.factor
    ));
    let terms = d
        .terms
        .iter()
        .map(|t| {
            format!(
                "{{\"quantity\":{},\"period\":{},\"sign\":{},\"role\":{}}}",
                js(&t.quantity),
                js(&t.period),
                t.sign,
                js(t.role.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"doc\":{},\"family\":{},\"kind\":{},\"total\":{},\"total_period\":{},\"unit\":{},\"basis\":{},\"construction\":{},\"convention\":{},\"replacement\":{replacement}{cost_rate},\"terms\":[{}]}}",
        js(&d.id),
        js(&d.doc),
        js(&d.family),
        js(d.kind.as_str()),
        js(&d.total),
        js(&d.total_period),
        js(&d.unit),
        js(&d.basis),
        js(&d.construction),
        js(&d.convention),
        terms
    )
}

fn certificate_json(certificate: &xf::Certificate) -> String {
    match certificate {
        xf::Certificate::Residual { identity, residual } => format!(
            "{{\"kind\":\"printed_residual\",\"identity\":{},\"residual\":{}}}",
            js(identity),
            rat(*residual)
        ),
        xf::Certificate::Farkas { multipliers } => {
            let rows = multipliers
                .iter()
                .map(|(id, v)| format!("{{\"identity\":{},\"value\":{}}}", js(id), rat(*v)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"kind\":\"farkas\",\"multipliers\":[{rows}]}}")
        }
    }
}

fn blame_json(blame: &xf::BlameOutcome) -> String {
    match blame {
        xf::BlameOutcome::Indeterminate { reason } => {
            format!("{{\"status\":\"indeterminate\",\"reason\":{}}}", js(reason))
        }
        xf::BlameOutcome::Blamed(blame) => {
            let label = match blame.label {
                xf::CorrectionLabel::Minimum => "minimum",
                xf::CorrectionLabel::InclusionMinimalOnly => "inclusion_minimal_only",
            };
            let conflicts = blame
                .conflicts
                .iter()
                .map(|s| {
                    format!(
                        "{{\"cells\":{},\"certificate\":{}}}",
                        strings(&s.cells),
                        certificate_json(&s.certificate)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let corrections = blame
                .corrections
                .iter()
                .map(|set| {
                    let implied = set
                        .implied
                        .iter()
                        .map(|(loc, result)| {
                            let value = match result {
                                Ok(iv) => format!("\"interval\":{}", interval(iv)),
                                Err(error) => {
                                    format!("\"unavailable\":{}", js(&format!("{error:?}")))
                                }
                            };
                            format!("{{\"locator\":{},{value}}}", js(loc))
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(
                        "{{\"cells\":{},\"implied\":[{implied}]}}",
                        strings(&set.cells)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"status\":\"blamed\",\"label\":{},\"conflicts\":[{conflicts}],\"corrections\":[{corrections}]}}",
                js(label)
            )
        }
    }
}

impl TableReport {
    fn body(&self) -> String {
        let metric_definitions = self.metric_definitions.iter().map(|d| format!(
            "{{\"id\":{},\"family\":\"reserve_replacement\",\"unit\":{},\"formula\":{},\"source\":{},\"evidence\":{},\"credit_basis\":{},\"extends\":{},\"supersedes\":{}}}",
            js(&d.id), js(&d.unit), js(&d.formula), js(&d.source), js(&d.evidence),
            js(match d.credit_basis { fmine_registry::metrics::CreditBasis::ByProduct => "by_product", fmine_registry::metrics::CreditBasis::CoProduct => "co_product" }),
            strings(&d.extends), d.supersedes.as_ref().map_or_else(|| "null".into(), |s| js(s))
        )).collect::<Vec<_>>().join(",");
        let definitions = self
            .definitions
            .iter()
            .map(definition_json)
            .collect::<Vec<_>>()
            .join(",");
        let cells = self.cells.iter().map(|s| {
            let d = &s.document;
            format!("{{\"source\":{},\"printed\":{},\"step\":{},\"document\":{{\"file\":{},\"sha256\":{},\"kind\":{},\"published_at\":{},\"published_at_basis\":{},\"pages\":{},\"bytes\":{},\"url\":{},\"url_basis\":{}}}}}",
                accepted_json(&s.source), rat(s.cell.printed), rat(s.cell.step), js(&d.file), js(&d.sha256), js(&d.kind), js(&d.published_at), js(&d.published_at_basis), d.pages, d.bytes, js(&d.url), js(&d.url_basis))
        }).collect::<Vec<_>>().join(",");
        let components = self.components.iter().map(|check| {
            let result = match &check.component.outcome {
                xf::Outcome::Feasible { witness } => {
                    let witness = witness.iter().map(|(loc, v)| format!("{{\"locator\":{},\"value\":{}}}", js(loc), rat(*v))).collect::<Vec<_>>().join(",");
                    format!("\"verdict\":\"feasible\",\"checked\":true,\"witness\":[{witness}]")
                }
                xf::Outcome::Infeasible { certificate } => format!("\"verdict\":\"infeasible\",\"checked\":true,\"certificate\":{}", certificate_json(certificate)),
                xf::Outcome::NotRefuted => "\"verdict\":\"not_refuted\",\"checked\":false".into(),
                xf::Outcome::Indeterminate { reason } => format!("\"verdict\":\"indeterminate\",\"checked\":false,\"reason\":{}", js(reason)),
            };
            let implied = check.implied.iter().map(|(loc, value)| {
                let result = match value { Ok(iv) => format!("\"interval\":{}", interval(iv)), Err(error) => format!("\"unavailable\":{}", js(error)) };
                format!("{{\"locator\":{},{result}}}", js(loc))
            }).collect::<Vec<_>>().join(",");
            format!("{{\"identities\":{},\"cells\":{},{result},\"certificate_digest\":{},\"blame\":{},\"implied\":[{implied}]}}",
                strings(&check.component.identities), strings(&check.component.cells), check.certificate_digest.as_ref().map_or_else(|| "null".into(), |s| js(s)),
                check.blame.as_ref().map_or_else(|| "null".into(), blame_json))
        }).collect::<Vec<_>>().join(",");
        let refused = self
            .refused
            .iter()
            .map(|r| {
                let evidence = r
                    .evidence
                    .iter()
                    .map(accepted_json)
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"identity\":{},\"doc\":{},\"reason\":{},\"evidence\":[{evidence}]}}",
                    js(&r.identity),
                    js(&r.doc),
                    js(&r.reason)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "\"schema\":{},\"issuer\":{},\"gate_eligible\":false,\"definitions\":[{definitions}],\"metric_definitions\":[{metric_definitions}],\"cells\":[{cells}],\"components\":[{components}],\"refused\":[{refused}],\"cutoffs\":{},\"no_claim\":{}",
            js(SCHEMA),
            js(&self.issuer),
            self.cutoffs.to_json(),
            js(NO_CLAIM)
        )
    }

    /// SHA-256 of the complete deterministic JSON body, excluding only this digest field.
    #[must_use]
    pub fn digest(&self) -> String {
        sha256(format!("{{{}}}", self.body()).as_bytes()).to_hex()
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!("{{{},\"digest\":{}}}", self.body(), js(&self.digest()))
    }
}

#[cfg(test)]
mod tests {
    include!("../../xtask/testdata/metrics_roll_forward_controls.rs");
}
