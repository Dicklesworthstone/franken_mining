//! Authenticated public peer comparisons and explicit competition-rank claims (BD-355).
//! Determinism: Exact

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::inspection::public_json;
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{EvidenceClass, Refusal, RefusalCode};
use fmine_metrics::comparison::{self, Metric, RankClaim, RankOrder, Request};
use fmine_public::sources::{DocumentRow, is_issuer_name, load_issuer_profile};
use fmine_public::transcribe::load_transcription_quantities_as_of;
use fmine_registry::sources::is_date;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::guidance_capacity::interval;
use crate::{Envelope, NextAction, json_string as j, worksheet_input};

const SCHEMA: &str = "fmine.peer-comparison.v1";
const BINDINGS_SCHEMA: &str = "fmine.peer-comparison.bindings.v1";
pub const USAGE: &str = "compare --metric aisc|gold-production --issuers a,b,c,d --period YYYYFY --as-of YYYY-MM-DD --bindings <path> [--claim-issuer <issuer> --claim lowest|highest|lowest:N|highest:N]";
pub const ROBOT_DOCS_JSON: &str = r#"{"bindings_schema":"fmine.peer-comparison.bindings.v1","bindings":"Only [[issuer]] rows with issuer, document and quantity; exactly the complete --issuers peer set; values and definitions never come from this file","source":"Each issuer's registered mda/aif/partner_filing and exactly one agreed actual transcription point; two distinct readers, SHA-256 authenticated bytes and FACT publication evidence","period":"Complete calendar year YYYYFY; actual publication must be at or after year end and no later than --as-of","metrics":{"aisc":"USD/oz with sold denominator","gold-production":"koz with produced denominator"},"basis_common":["metric=aisc|gold-production","scope=company","ownership=full|attributable","operations=all|continuing","metal=gold","denominator=sold|produced"],"aisc_definition":["allocation=by_product|co_product","credits=separate_line|inside_selling_costs|excluded","inventory=included|excluded","ga=included|excluded","sustaining_leases=included|excluded","minesite_exploration=included|excluded","reclamation=operating_only|all_sites|excluded","stripping=included|excluded","accounting=as_reported|historical_cost|purchase_price_allocation"],"definition_citation":["definition_page","definition_table","definition_row","definition_col"],"definition_encoding":"Semicolon-separated key=value fields in the reader-agreed transcription basis; same selected document; every semantic definition field must match across peers","ranks":"Exact closed half-step rounding boxes; competition rank, ties share rank and subsequent ranks skip tied positions; ascending and descending bounds retained","claims":"--claim-issuer and --claim must occur together; lowest:N/highest:N means competition rank at most N, lowest/highest means N=1; tied issuers may all satisfy rank1","limits":"2..64 peers, no dropped peers, transferred bridges, unit/ownership conversions, arbitrary prose interpretation or proprietary industry quartiles"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub request: Request,
    pub bindings: PathBuf,
}

fn bad(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("peer comparison: {}", reason.into()))
}

/// The common router removes workspace and output-format options.
pub fn parse(words: &[String]) -> Result<Command, String> {
    if words.first().map(String::as_str) != Some("compare") {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut i = 1;
    while i < words.len() {
        let flag = words[i].as_str();
        if !matches!(
            flag,
            "--metric"
                | "--issuers"
                | "--period"
                | "--as-of"
                | "--bindings"
                | "--claim-issuer"
                | "--claim"
        ) {
            return Err(format!("compare: unsupported option {flag}; {USAGE}"));
        }
        let value = words
            .get(i + 1)
            .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("compare: missing value for {flag}"))?;
        if fields.insert(flag, value.as_str()).is_some() {
            return Err(format!("compare: duplicate {flag}"));
        }
        i += 2;
    }
    let required = |key| {
        fields
            .get(key)
            .copied()
            .ok_or_else(|| format!("compare requires {key}"))
    };
    let metric = match required("--metric")? {
        "aisc" => Metric::Aisc,
        "gold-production" => Metric::GoldProduction,
        _ => return Err("compare --metric must be aisc or gold-production".into()),
    };
    let period = required("--period")?;
    let year = period
        .strip_suffix("FY")
        .filter(|s| s.len() == 4)
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|year| (1900..=9999).contains(year))
        .ok_or("compare --period requires a complete calendar year YYYYFY")?;
    let as_of = required("--as-of")?;
    if !is_date(as_of) {
        return Err("compare --as-of requires YYYY-MM-DD".into());
    }
    let issuers = required("--issuers")?
        .split(',')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let unique = issuers.iter().collect::<BTreeSet<_>>();
    if !(2..=comparison::MAX_PEERS).contains(&issuers.len())
        || unique.len() != issuers.len()
        || issuers
            .iter()
            .any(|issuer| !is_issuer_name(issuer) || issuer == "synthetic")
    {
        return Err(
            "compare --issuers requires 2..64 distinct registered issuer keys separated by commas"
                .into(),
        );
    }
    let claim = match (fields.get("--claim-issuer"), fields.get("--claim")) {
        (None, None) => None,
        (Some(issuer), Some(text)) => {
            let (order, threshold) = text.split_once(':').unwrap_or((*text, "1"));
            let order = match order {
                "lowest" => RankOrder::Lowest,
                "highest" => RankOrder::Highest,
                _ => {
                    return Err(
                        "compare --claim requires lowest, highest, lowest:N or highest:N".into(),
                    );
                }
            };
            let at_most = threshold
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=issuers.len()).contains(n))
                .ok_or("compare claim threshold must be within the declared peer count")?;
            if !issuers
                .iter()
                .any(|candidate| candidate.as_str() == *issuer)
            {
                return Err("compare --claim-issuer must be in the declared peer set".into());
            }
            Some(RankClaim {
                issuer: (*issuer).into(),
                order,
                at_most,
            })
        }
        _ => return Err("compare --claim-issuer and --claim are required together".into()),
    };
    Ok(Command {
        request: Request {
            metric,
            year,
            as_of: as_of.into(),
            issuers,
            claim,
        },
        bindings: PathBuf::from(required("--bindings")?),
    })
}

#[derive(Debug, Clone)]
struct Binding {
    issuer: String,
    document: String,
    quantity: String,
}

fn strings(fields: &[TomlField], allowed: &[&str]) -> Result<BTreeMap<String, String>, Refusal> {
    let mut values = BTreeMap::new();
    for field in fields {
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(
                RefusalCode::NotObservable202,
                "bindings fields must be quoted strings",
            ));
        };
        if !allowed.contains(&field.key.as_str())
            || value.trim().is_empty()
            || values.insert(field.key.clone(), value.clone()).is_some()
        {
            return Err(bad(
                RefusalCode::NotObservable202,
                format!("unknown, empty or duplicate binding field {}", field.key),
            ));
        }
    }
    for field in allowed {
        if !values.contains_key(*field) {
            return Err(bad(
                RefusalCode::NotObservable202,
                format!("missing binding field {field}"),
            ));
        }
    }
    Ok(values)
}

fn bindings(text: &str, request: &Request) -> Result<Vec<Binding>, Refusal> {
    let doc = parse_subset(text).map_err(|e| bad(RefusalCode::NotObservable202, e.to_string()))?;
    let top = strings(&doc.top, &["schema"])?;
    if top.get("schema").map(String::as_str) != Some(BINDINGS_SCHEMA) {
        return Err(bad(
            RefusalCode::NotObservable202,
            format!("bindings require {BINDINGS_SCHEMA}"),
        ));
    }
    let mut bindings = BTreeMap::new();
    for row in &doc.rows {
        if row.table != "issuer" {
            return Err(bad(
                RefusalCode::NotObservable202,
                "bindings only admit [[issuer]] selector rows",
            ));
        }
        let mut values = strings(&row.fields, &["issuer", "document", "quantity"])?;
        let mut take = |key| {
            values
                .remove(key)
                .ok_or_else(|| bad(RefusalCode::NotObservable202, format!("missing {key}")))
        };
        let binding = Binding {
            issuer: take("issuer")?,
            document: take("document")?,
            quantity: take("quantity")?,
        };
        if bindings.insert(binding.issuer.clone(), binding).is_some() {
            return Err(bad(
                RefusalCode::NotObservable202,
                "bindings contain a duplicate issuer",
            ));
        }
    }
    let expected: BTreeSet<_> = request.issuers.iter().cloned().collect();
    if bindings.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(bad(
            RefusalCode::NotObservable202,
            "bindings must retain exactly the entire declared peer set",
        ));
    }
    Ok(bindings.into_values().collect())
}

fn anchor(source: &SourceAnchor) -> String {
    format!(
        "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
        j(&source.artifact_path),
        j(&Digest32(source.artifact_sha256).to_hex()),
        j(&source.locator)
    )
}

fn definition(definition: &comparison::Definition) -> String {
    format!(
        "{{{}}}",
        definition
            .fields
            .iter()
            .map(|(key, value)| format!("{}:{}", j(key), j(value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn reading(value: &comparison::PeerReading) -> String {
    format!(
        "{{\"issuer\":{},\"metric\":{},\"period\":{},\"unit\":{},\"interval\":{},\"definition\":{},\"source\":{},\"definition_source\":{},\"quantity\":{},\"raw_token\":{},\"basis\":{},\"published_at\":{},\"published_at_basis\":{},\"readers\":[{}]}}",
        j(&value.issuer),
        j(value.metric.as_str()),
        j(&format!("{}FY", value.year)),
        j(value.metric.unit()),
        interval(value.interval),
        definition(&value.definition),
        anchor(&value.source),
        anchor(&value.definition_source),
        j(&value.quantity),
        j(&value.raw_token),
        j(&value.basis),
        j(&value.published_at),
        j(&value.published_at_basis),
        value
            .readers
            .iter()
            .map(|r| j(r))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn summary(value: &comparison::Summary) -> String {
    let peers = value.peers.iter().map(|peer| format!("{{\"issuer\":{},\"source_claim\":{},\"interval\":{},\"ascending_rank\":{{\"min\":{},\"max\":{}}},\"descending_rank\":{{\"min\":{},\"max\":{}}}}}",
        j(&peer.issuer), j(&peer.source_claim.to_string()), interval(peer.interval), peer.ascending.min, peer.ascending.max, peer.descending.min, peer.descending.max)).collect::<Vec<_>>().join(",");
    let claim = value.request.claim.as_ref().map_or("null".into(), |claim| {
        format!(
            "{{\"issuer\":{},\"order\":{},\"competition_rank_at_most\":{},\"verdict\":{}}}",
            j(&claim.issuer),
            j(claim.order.as_str()),
            claim.at_most,
            value.verdict.map_or("null".into(), |v| j(v.as_str()))
        )
    });
    format!(
        "{{\"metric\":{},\"period\":{},\"as_of\":{},\"unit\":{},\"peer_count\":{},\"declared_issuers\":[{}],\"rank_convention\":\"competition: ties share rank\",\"definition\":{},\"peers\":[{peers}],\"comparative_claim\":{claim}}}",
        j(value.request.metric.as_str()),
        j(&format!("{}FY", value.request.year)),
        j(&value.request.as_of),
        j(value.request.metric.unit()),
        value.peers.len(),
        value
            .request
            .issuers
            .iter()
            .map(|issuer| j(issuer))
            .collect::<Vec<_>>()
            .join(","),
        definition(&value.definition)
    )
}

fn markdown(report: &comparison::Report) -> String {
    let value = &report.claim.value;
    let mut text = format!(
        "# Public peer comparison\n\n{}\n\nMetric: **{}** ({}), complete year **{}**, evidence cut **{}**. Complete selected population: **{} issuers**.\n\nCompetition ranks share position for equal values; for example, two tied lowest values both rank 1 and the next ranks 3. Bounds are exact possibilities over closed display-rounding intervals. A lowest/highest claim includes ties.\n\n| Issuer | Printed value | Rounding bounds | Ascending rank | Descending rank | Value citation |\n|---|---|---|---|---|---|\n",
        comparison::NO_CLAIM,
        value.request.metric.as_str(),
        value.request.metric.unit(),
        value.request.year,
        value.request.as_of,
        value.peers.len()
    );
    let safe = |s: &str| s.replace('|', "\\|").replace(['\r', '\n'], " ");
    for (peer, input) in value.peers.iter().zip(&report.inputs) {
        let reading = &input.value;
        let _ = writeln!(
            text,
            "| {} | {} | [{}/{}, {}/{}] | {}–{} | {}–{} | {} |",
            safe(&peer.issuer),
            safe(&reading.raw_token),
            peer.interval.lo.num(),
            peer.interval.lo.den(),
            peer.interval.hi.num(),
            peer.interval.hi.den(),
            peer.ascending.min,
            peer.ascending.max,
            peer.descending.min,
            peer.descending.max,
            safe(&reading.source.locator)
        );
    }
    if let Some(claim) = &value.request.claim {
        let _ = writeln!(
            text,
            "\nComparative claim: **{}** has {} competition rank at most {}: **{}**.",
            safe(&claim.issuer),
            claim.order.as_str(),
            claim.at_most,
            value
                .verdict
                .map_or("unavailable", comparison::Verdict::as_str)
        );
    }
    text.push_str("\n## Matched definitions\n\n");
    for (field, treatment) in &value.definition.fields {
        let _ = writeln!(text, "- {field}: {treatment}");
    }
    text.push_str("\n## Source and definition provenance\n\n");
    for input in &report.inputs {
        let v = &input.value;
        let _ = writeln!(
            text,
            "- **{}**: source Claim `{}`; document SHA-256 `{}`; definition {}; published {} ({}); readers {}.",
            safe(&v.issuer),
            input.claim_id,
            Digest32(v.source.artifact_sha256),
            safe(&v.definition_source.locator),
            v.published_at,
            safe(&v.published_at_basis),
            v.readers
                .iter()
                .map(|r| safe(r))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let _ = writeln!(
        text,
        "\nDerived comparison Claim: `{}`; exact result digest `{}`.",
        report.claim.claim_id,
        report.claim.value_tag()
    );
    text
}

fn document_json(issuer: &str, document: &DocumentRow) -> String {
    format!(
        "{{\"issuer\":{},\"file\":{},\"url\":{},\"sha256\":{},\"bytes\":{},\"published_at\":{},\"published_at_basis\":{}}}",
        j(issuer),
        j(&document.file),
        j(&document.url),
        j(&document.sha256),
        document.bytes,
        j(&document.published_at),
        j(&document.published_at_basis)
    )
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, selection_source) = worksheet_input::read(
            &root.join(&command.bindings),
            "peer comparison bindings",
            1024 * 1024,
        )?;
        let bindings = bindings(&text, &command.request)?;
        let mut inputs = Vec::new();
        let mut documents = Vec::new();
        for binding in bindings {
            let profile = load_issuer_profile(root, &binding.issuer)?;
            let selected = profile
                .documents
                .iter()
                .filter(|d| d.file == binding.document)
                .collect::<Vec<_>>();
            let [document] = selected.as_slice() else {
                return Err(bad(
                    RefusalCode::NoClaimInDomain201,
                    format!(
                        "{} needs exactly one registered document {}",
                        binding.issuer, binding.document
                    ),
                ));
            };
            if document.published_at > command.request.as_of {
                return Err(bad(
                    RefusalCode::AssayLookahead108,
                    format!(
                        "{}/{} is after the evidence cut",
                        binding.issuer, binding.document
                    ),
                ));
            }
            let loaded = load_transcription_quantities_as_of(
                &profile,
                root,
                &command.request.as_of,
                std::slice::from_ref(&binding.quantity),
                Some(&binding.document),
            )?
            .ok_or_else(|| {
                bad(
                    RefusalCode::NotObservable202,
                    format!(
                        "{} has no transcription registry; no peer may be dropped",
                        binding.issuer
                    ),
                )
            })?;
            if !loaded.agreement.is_consistent() || !loaded.agreement.single_reader.is_empty() {
                return Err(bad(
                    RefusalCode::NotObservable202,
                    format!(
                        "{} needs complete independent reader agreement",
                        binding.issuer
                    ),
                ));
            }
            let [row] = loaded.agreement.accepted.as_slice() else {
                return Err(bad(
                    RefusalCode::NotObservable202,
                    format!(
                        "{} needs exactly one agreed actual for {}; missing or ambiguous peers are retained as refusal",
                        binding.issuer, binding.quantity
                    ),
                ));
            };
            if row.quantity != binding.quantity {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "accepted quantity differs from its explicit selector",
                ));
            }
            inputs.push(comparison::source::bind(
                &binding.issuer,
                command.request.metric,
                command.request.year,
                document,
                row,
                EvidenceClass::ISSUER,
            )?);
            documents.push(document_json(&binding.issuer, document));
        }
        let report = comparison::compare(&command.request, &inputs)?;
        let source_claims = report
            .inputs
            .iter()
            .map(|input| public_json(input, "peer_comparison_source", reading(&input.value)))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let derived = public_json(
            &report.claim,
            "peer_comparison_result",
            summary(&report.claim.value),
        )?;
        let payload = format!(
            "{{\"schema\":\"{SCHEMA}\",\"no_claim\":{},\"bindings\":{},\"documents\":[{}],\"source_claims\":[{source_claims}],\"result\":{derived}}}",
            j(comparison::NO_CLAIM),
            worksheet_input::source_json(&selection_source, text.len()),
            documents.join(",")
        );
        let mut envelope = Envelope::ok("compare", payload, Vec::new());
        envelope.no_claim = Some(comparison::NO_CLAIM);
        envelope.rendered_markdown = Some(markdown(&report));
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        let mut envelope = Envelope::refused("compare", refusal, vec![NextAction::new("fmine robot-docs", "inspect the peer selectors, agreed source definitions and competition-rank semantics")]);
        envelope.payload_json = format!("{{\"schema\":\"{SCHEMA}\",\"metric\":{},\"period\":{},\"as_of\":{},\"declared_peer_count\":{},\"declared_issuers\":[{}],\"comparison\":null}}",
            j(command.request.metric.as_str()), j(&format!("{}FY", command.request.year)), j(&command.request.as_of), command.request.issuers.len(),
            command.request.issuers.iter().map(|issuer| j(issuer)).collect::<Vec<_>>().join(","));
        envelope.no_claim = Some(comparison::NO_CLAIM);
        envelope
    })
}
