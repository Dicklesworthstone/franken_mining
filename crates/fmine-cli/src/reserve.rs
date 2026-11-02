//! Complete annual reserve-replacement reports (BD-063). The CLI authenticates selected documents;
//! the pure metric engine retains source assertions and computes exact cumulative ratios.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::reserve_replacement::{
    self, Bounds, ReplacementReport, SeriesAssessment, Totals,
};
use fmine_public::sources::{
    is_issuer_name, load_issuer_profile, registry_path, transcriptions_path,
};
use fmine_public::transcribe::{check_docs, load_transcription_quantities_as_of};
use fmine_registry::metrics::MetricRegistry;
use fmine_registry::reserve_replacement::parse_replacement_registry;
use fmine_registry::sources::{DocumentRow, is_date};
use fmine_registry::transcriptions::{Accepted, Locator};

use crate::guidance::{number, text};
use crate::{Envelope, NextAction, OutputFormat, escape_json};

const VERB: &str = "reserve replacement";
const NO_CLAIM: &str = "Recomputes the declared reserve-replacement definitions from registered issuer assertions. \
    Closed half-last-digit rounding intervals are arithmetic assumptions, not a statistical confidence interval. \
    Approximate headlines supply no numerical tolerance. These checks do not verify physical reserves, classify \
    resources, certify disclosure, establish a complete historical information set or imply issuer affiliation.";

pub(crate) const VERB_USAGE: &str = "reserve replacement --issuer <issuer> --bindings <path> [--as-of YYYY-MM-DD] [--format json|markdown]";
pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"inputs":"explicit complete annual selectors, optional aggregate totals and separately qualified headline, agreed PDF readings and registered metric rules","arithmetic":"ratio of cumulative net changes to explicitly signed depletion; exact rationals and joint rounding constraints","basis":"metal, ownership, excluded acquisitions/divestments, annual window and GEO price decks remain explicit","availability":"--as-of rejects declared documents after the cut; this retrospective report does not certify a complete hindcast information set","refusal":"missing or invalid series remains in the report and yields a nonzero exit; incompatible rounded headlines remain reported findings","authority":"issuer assertions only; no reserve classification or disclosure authority"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    issuer: String,
    bindings: PathBuf,
    as_of: Option<String>,
}

pub(crate) fn parse(
    args: &[String],
    default_root: &Path,
) -> Result<(Command, OutputFormat, PathBuf), String> {
    let mut fields = BTreeMap::new();
    let mut words = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let (name, value) = match arg {
            "--json" => ("--format", "json".to_string()),
            "--issuer" | "--bindings" | "--as-of" | "--format" | "--workspace" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| format!("missing {arg} value"))?;
                if value.trim().is_empty() || value.starts_with("--") {
                    return Err(format!("missing {arg} value"));
                }
                (arg, value.clone())
            }
            other if other.starts_with('-') => return Err(format!("unsupported option {other}")),
            word => {
                words.push(word);
                i += 1;
                continue;
            }
        };
        if fields.insert(name, value).is_some() {
            return Err(format!("{name} may be supplied only once"));
        }
        i += 1;
    }
    if words != ["reserve", "replacement"] {
        return Err("expected reserve replacement".into());
    }
    let workspace = fields
        .remove("--workspace")
        .map_or_else(|| default_root.to_path_buf(), PathBuf::from);
    let issuer = fields.remove("--issuer").ok_or("missing --issuer")?;
    if !is_issuer_name(&issuer) || !registry_path(&workspace, &issuer).is_file() {
        return Err(format!("unknown issuer {issuer:?}"));
    }
    let bindings = PathBuf::from(fields.remove("--bindings").ok_or("missing --bindings")?);
    let as_of = fields.remove("--as-of");
    if as_of.as_deref().is_some_and(|date| !is_date(date)) {
        return Err("--as-of requires YYYY-MM-DD".into());
    }
    let format = match fields.remove("--format").as_deref().unwrap_or("json") {
        "json" => OutputFormat::Json,
        "markdown" | "md" => OutputFormat::Markdown,
        _ => return Err("--format requires json or markdown".into()),
    };
    Ok((
        Command {
            issuer,
            bindings,
            as_of,
        },
        format,
        workspace,
    ))
}

fn js(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine doctor",
        "inspect registered source availability",
    )]
}

fn read(path: &Path, what: &str) -> Result<String, Refusal> {
    std::fs::read_to_string(path).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{what} {}: {e}", path.display()),
        )
    })
}

pub(crate) fn execute(command: &Command, root: &Path) -> Envelope {
    execute_inner(command, root).unwrap_or_else(|r| Envelope::refused(VERB, r, actions()))
}

fn execute_inner(command: &Command, root: &Path) -> Result<Envelope, Refusal> {
    let profile = load_issuer_profile(root, &command.issuer)?;
    if profile.issuer != command.issuer {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            format!(
                "source registry issuer {} does not match requested issuer {}",
                profile.issuer, command.issuer
            ),
        ));
    }
    let path = if command.bindings.is_absolute() {
        command.bindings.clone()
    } else {
        root.join(&command.bindings)
    };
    let bindings_text = read(&path, "reserve bindings")?;
    let bindings = parse_replacement_registry(&bindings_text, &command.issuer).map_err(|e| {
        Refusal::new(
            RefusalCode::NotObservable202,
            format!("reserve bindings: {e}"),
        )
    })?;
    let metrics_text = read(&root.join("registries/metrics.toml"), "metric definitions")?;
    let metrics = MetricRegistry::parse(&metrics_text).map_err(|e| {
        Refusal::new(
            RefusalCode::NotObservable202,
            format!("metric definitions: {e}"),
        )
    })?;

    let mut quantities: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for series in &bindings.series {
        for year in &series.years {
            let keys = quantities.entry(&year.doc).or_default();
            keys.extend([year.net_change.clone(), year.depletion.clone()]);
        }
        if let Some(aggregate) = &series.aggregate {
            quantities
                .entry(&aggregate.doc)
                .or_default()
                .extend([aggregate.net_change.clone(), aggregate.depletion.clone()]);
        }
        if let Some(headline) = &series.headline {
            quantities
                .entry(&headline.doc)
                .or_default()
                .insert(headline.quantity.clone());
        }
    }
    // Check the entire declared source population before opening any PDF. A missing reading cannot
    // hide a post-cut document or shrink the denominator to the remaining usable years.
    for doc in quantities.keys() {
        let selected: Vec<_> = profile
            .documents
            .iter()
            .filter(|d| d.file == *doc)
            .collect();
        let [document] = selected.as_slice() else {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!("reserve selector {doc} must name exactly one registered document"),
            ));
        };
        if command
            .as_of
            .as_deref()
            .is_some_and(|cut| document.published_at.as_str() > cut)
        {
            return Err(Refusal::new(
                RefusalCode::TemporalConeViolation110,
                format!(
                    "reserve document {doc} is public from {}, after the requested evidence cut",
                    document.published_at
                ),
            ));
        }
    }
    let mut accepted = Vec::new();
    let mut pending = Vec::new();
    let mut checked = BTreeSet::new();
    for (doc, keys) in &quantities {
        let keys: Vec<_> = keys.iter().cloned().collect();
        let Some(report) = load_transcription_quantities_as_of(
            &profile,
            root,
            command.as_of.as_deref().unwrap_or("9999-12-31"),
            &keys,
            Some(doc),
        )?
        else {
            return Err(Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!(
                    "reserve operands require {}",
                    transcriptions_path(root, &command.issuer).display()
                ),
            ));
        };
        let clashes = report.agreement.accepted_pending_clashes();
        if !clashes.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!(
                    "reserve quantity has competing pending readings: {}",
                    clashes.join("; ")
                ),
            ));
        }
        checked.extend(report.documents.into_iter().map(|d| d.file));
        accepted.extend(report.agreement.accepted);
        pending.extend(report.agreement.single_reader);
    }
    let unchecked: Vec<_> = profile
        .documents
        .iter()
        .filter(|d| quantities.contains_key(d.file.as_str()) && !checked.contains(&d.file))
        .collect();
    check_docs(&unchecked, root)?;
    accepted.sort_by(|a, b| a.locator.cmp(&b.locator));
    pending.sort();
    let report = reserve_replacement::assess(&bindings, &metrics, &accepted, &profile.documents);
    let pending_json = pending
        .iter()
        .map(|(at, reader)| {
            format!(
                "{{\"locator\":{},\"reader\":{}}}",
                js(&at.to_string()),
                js(reader)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let as_of = command.as_of.as_deref().map_or_else(|| "null".into(), js);
    let body = format!(
        "{{\"schema\":\"fmine.reserve-replacement.report.v1\",\"lane\":\"public\",\"gate_eligible\":false,\
         \"issuer\":{},\"bindings_sha256\":{},\"rules_sha256\":{},\"as_of\":{as_of},\
         \"checks\":{},\"pending_readings\":[{pending_json}],\"no_claim\":{}}}",
        js(&command.issuer),
        js(&sha256(bindings_text.as_bytes()).to_hex()),
        js(&sha256(metrics_text.as_bytes()).to_hex()),
        report.to_json(),
        js(NO_CLAIM),
    );
    let digest = sha256(format!("fmine.reserve-replacement.report.v1\0{body}").as_bytes()).to_hex();
    let payload = format!("{{\"digest\":{},{}", js(&digest), &body[1..]);
    let mut envelope = if report.registry_refusal.is_some()
        || report
            .series
            .iter()
            .any(|s| matches!(s, SeriesAssessment::Refused(_)))
    {
        Envelope::refused(
            VERB,
            Refusal::new(
                RefusalCode::NotObservable202,
                "one or more declared reserve-replacement series cannot be computed; see retained checks",
            ),
            actions(),
        )
    } else {
        Envelope::ok(VERB, payload.clone(), actions())
    };
    envelope.payload_json = payload;
    envelope.no_claim = Some(NO_CLAIM);
    envelope.rendered_markdown = Some(markdown(
        &report,
        &pending,
        command.as_of.as_deref(),
        &digest,
    ));
    Ok(envelope)
}

fn bounds(value: Bounds) -> String {
    format!("{} to {}", number(value.lo), number(value.hi))
}

fn total_row(out: &mut String, name: &str, totals: &Totals) {
    let _ = writeln!(
        out,
        "| {name} | {} | {} | {} | {} |",
        number(totals.net_change.printed),
        number(totals.depletion.printed),
        number(totals.percent.printed),
        bounds(totals.percent.bounds)
    );
}

fn evidence_row(out: &mut String, role: &str, reading: &Accepted, doc: Option<&DocumentRow>) {
    let _ = writeln!(
        out,
        "| {} | {} | {} | {} | {} | {} | `{}` | {} |",
        text(role),
        text(&reading.token),
        text(&reading.unit),
        text(&reading.basis),
        text(&reading.period),
        text(&reading.locator.to_string()),
        text(&reading.doc_sha256),
        text(&reading.readers.join(", "))
    );
    if let Some(doc) = doc {
        let _ = writeln!(
            out,
            "\nSource availability: {} — {}.\n",
            text(&doc.published_at),
            text(&doc.published_at_basis)
        );
    }
}

fn evidence_header(out: &mut String) {
    out.push_str(
        "\n| Role | Printed token | Unit | Basis | Period | Locator | SHA-256 | Reader roles |\n\
                  |---|---|---|---|---|---|---|---|\n",
    );
}

fn markdown(
    report: &ReplacementReport,
    pending: &[(Locator, String)],
    as_of: Option<&str>,
    digest: &str,
) -> String {
    let mut out = format!(
        "# Reserve replacement — {}\n\nReport digest: `{digest}`.\n\n{}\n",
        text(&report.issuer),
        NO_CLAIM
    );
    if let Some(cut) = as_of {
        let _ = writeln!(
            out,
            "\nDeclared-document availability cut: {}. Current registered rules apply; this is not a complete hindcast information set.",
            text(cut)
        );
    }
    if let Some(reason) = &report.registry_refusal {
        let _ = writeln!(out, "\nRegistry refusal: {}", text(reason));
    }
    if !pending.is_empty() {
        out.push_str("\n## Pending readings\n\nThese locations lack an agreeing second reading.\n\n| Locator | Reader role |\n|---|---|\n");
        for (locator, reader) in pending {
            let _ = writeln!(out, "| {} | {} |", text(&locator.to_string()), text(reader));
        }
    }
    for result in &report.series {
        match result {
            SeriesAssessment::Refused(refused) => {
                let _ = writeln!(
                    out,
                    "\n## {}\n\nRefused: {}\n",
                    text(&refused.selection.id),
                    text(&refused.reason)
                );
                evidence_header(&mut out);
                for reading in &refused.evidence {
                    evidence_row(&mut out, &reading.quantity, reading, None);
                }
                for doc in &refused.documents {
                    let _ = writeln!(
                        out,
                        "\nSource {}: `{}`, available {} — {}.",
                        text(&doc.file),
                        text(&doc.sha256),
                        text(&doc.published_at),
                        text(&doc.published_at_basis)
                    );
                }
            }
            SeriesAssessment::Computed(value) => {
                let s = &value.selection;
                let _ = writeln!(
                    out,
                    "\n## {}\n\nDefinition: `{}`. Years: {}–{}. Amounts: {}; basis: {}. Depletion source convention: {}.\n\nDefinition source: {} ({}) — {}.\n",
                    text(&s.id),
                    text(&value.definition.id),
                    s.start_year,
                    s.end_year,
                    text(&s.unit),
                    text(&s.basis),
                    s.depletion_sign.as_str(),
                    text(&value.definition.source),
                    text(&value.definition.evidence),
                    text(&value.definition.formula)
                );
                let _ = writeln!(
                    out,
                    "| View | Net change ({}) | Positive depletion ({}) | Replacement (%) | Closed rounding enclosure (%) |\n|---|---|---|---|---|",
                    text(&s.unit),
                    text(&s.unit)
                );
                total_row(&mut out, "Annual printed sums", &value.annual);
                if let Some(aggregate) = &value.aggregate {
                    total_row(&mut out, "Separate printed aggregate", &aggregate.totals);
                }
                let _ = writeln!(
                    out,
                    "\nJoint annual/aggregate rounding enclosure: **{}%**. No distinguished point is inferred from that intersection.\n",
                    bounds(value.joint.percent)
                );
                if let Some(headline) = &value.headline {
                    let _ = writeln!(
                        out,
                        "Headline: {}%; qualification: {}; assessment: `{}`.\n",
                        text(&headline.reading.source.token),
                        headline.kind.as_str(),
                        headline.state.as_str()
                    );
                    if headline.kind
                        == fmine_registry::reserve_replacement::HeadlineKind::Approximate
                    {
                        out.push_str("Approximate language supplies no numerical tolerance; comparison interval is absent.\n");
                    }
                }
                evidence_header(&mut out);
                for year in &value.years {
                    evidence_row(
                        &mut out,
                        &format!("{} net change", year.year),
                        &year.net_change.source,
                        None,
                    );
                    evidence_row(
                        &mut out,
                        &format!("{} signed depletion", year.year),
                        &year.depletion.source,
                        None,
                    );
                }
                if let Some(aggregate) = &value.aggregate {
                    evidence_row(
                        &mut out,
                        "aggregate net change",
                        &aggregate.net_change.source,
                        None,
                    );
                    evidence_row(
                        &mut out,
                        "aggregate signed depletion",
                        &aggregate.depletion.source,
                        None,
                    );
                }
                if let Some(headline) = &value.headline {
                    evidence_row(&mut out, "headline", &headline.reading.source, None);
                }
                let mut documents = BTreeMap::new();
                for year in &value.years {
                    documents.insert(&year.net_change.document.file, &year.net_change.document);
                    documents.insert(&year.depletion.document.file, &year.depletion.document);
                }
                if let Some(aggregate) = &value.aggregate {
                    documents.insert(
                        &aggregate.net_change.document.file,
                        &aggregate.net_change.document,
                    );
                    documents.insert(
                        &aggregate.depletion.document.file,
                        &aggregate.depletion.document,
                    );
                }
                if let Some(headline) = &value.headline {
                    documents.insert(&headline.reading.document.file, &headline.reading.document);
                }
                for doc in documents.values() {
                    let _ = writeln!(
                        out,
                        "\nSource {}: `{}`, available {} — {}.",
                        text(&doc.file),
                        text(&doc.sha256),
                        text(&doc.published_at),
                        text(&doc.published_at_basis)
                    );
                }
                if let Some(definition) = &value.geo_definition {
                    let _ = writeln!(
                        out,
                        "\nGEO definition: `{}` — {} ({}). Reported GEO is already converted; the retained decks are not applied again.\n\n| Year | Gold USD/oz | Copper USD/lb |\n|---|---|---|",
                        text(&definition.id),
                        text(&definition.source),
                        text(&definition.evidence)
                    );
                    for year in &value.years {
                        if let Some(deck) = &year.geo_deck {
                            let _ = writeln!(
                                out,
                                "| {} | {} | {} |",
                                year.year,
                                text(&deck.gold_usd_per_oz),
                                text(&deck.copper_usd_per_lb)
                            );
                        }
                    }
                }
            }
        }
    }
    out
}
