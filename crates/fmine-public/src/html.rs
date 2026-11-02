//! Registered HTML cells projected into canonical public Claims (BD-057/BD-059).
//! Mappings name exact labels, not amounts or semantic overrides. Every interpretation keeps
//! its source cell and the mapping bytes that selected it; figures remain issuer assertions.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::numtok::NumberToken;
use fmine_io::xlsx::parse_a1_ref;
use fmine_io::{HTML_MAX_BYTES, HtmlDocument, HtmlSourceCell};
use fmine_registry::sources::{HtmlSourceRow, is_date, parse_sources_registry};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::key::{
    KeyedCell, LineKey, NilReason, OwnershipBasis, PublicKey, PublicValue, Scope, ShareSource,
    StatusKind, Vintage, claims_digest, parse_period_code, parse_sheet_period,
    public_claim_anchored, scope_for,
};
use crate::sources::IssuerProfile;

pub const SCHEMA: &str = "fmine.html-claims.v1";
const MAX_DEFINITIONS_BYTES: usize = 1_048_576;
const MAX_CLAIMS: usize = 4096;
const ROLES: [&str; 5] = ["subject", "period", "metric", "unit", "ownership"];

/// An original cell and the requested slot, which may be covered by a merged origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlCellCitation {
    pub anchor: SourceAnchor,
    pub requested_locator: String,
    pub byte_range: (usize, usize),
    pub text: String,
    pub row_span: usize,
    pub column_span: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInterpretation {
    pub role: &'static str,
    pub value: String,
    pub source: HtmlCellCitation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HtmlClaim {
    pub definition_id: String,
    pub cell: KeyedCell,
    pub value_source: HtmlCellCitation,
    pub interpretations: Vec<HtmlInterpretation>,
    pub mapping_source: SourceAnchor,
    /// Exact printed decimal and step, retained even if a registered status makes zero Nil.
    pub exact_number: Option<NumberToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HtmlClaimsReport {
    pub issuer: String,
    pub definitions_path: String,
    pub definitions_sha256: String,
    pub registry_sha256: String,
    pub as_of: Option<String>,
    /// Only mapped, eligible sources; all were opened and their bytes verified.
    pub sources: Vec<HtmlSourceRow>,
    /// Mapped sources dated after the cut. Their files were not opened or inspected.
    pub excluded_sources: Vec<HtmlSourceRow>,
    pub claims: Vec<HtmlClaim>,
    pub claims_digest: Digest32,
}

struct Mapping {
    id: String,
    file: String,
    table: usize,
    cell: String,
    labels: Vec<(String, String)>,
    line: usize,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("HTML Claims: {}", detail.into()),
    )
}

fn only(fields: &[TomlField], allowed: &[&str]) -> Result<(), Refusal> {
    for field in fields {
        if !allowed.contains(&field.key.as_str()) {
            return Err(invalid(format!(
                "line {}: unknown field {}",
                field.line, field.key
            )));
        }
    }
    Ok(())
}

fn string(fields: &[TomlField], key: &str) -> Result<String, Refusal> {
    match fields.iter().find(|field| field.key == key) {
        Some(TomlField {
            value: TomlValue::Str(value),
            ..
        }) if !value.trim().is_empty()
            && value.len() <= 4096
            && !value.chars().any(char::is_control) =>
        {
            Ok(value.clone())
        }
        _ => Err(invalid(format!(
            "{key} must be a nonblank bounded string without controls"
        ))),
    }
}

fn coordinates(cell: &str) -> Result<(usize, usize), Refusal> {
    let Some((row, column)) = parse_a1_ref(cell) else {
        return Err(invalid(format!("{cell:?} must be one canonical A1 cell")));
    };
    if row == 0
        || row > 16_384
        || column == 0
        || column > 4096
        || format!("{}{row}", fmine_io::xlsx::col_index_to_letters(column)) != cell
    {
        return Err(invalid(format!(
            "{cell:?} is outside the bounded HTML grid"
        )));
    }
    Ok((row as usize, column as usize))
}

fn mappings(text: &str, issuer: &str, profile: &IssuerProfile) -> Result<Vec<Mapping>, Refusal> {
    let doc = parse_subset(text).map_err(|error| invalid(error.to_string()))?;
    only(&doc.top, &["schema", "issuer"])?;
    if string(&doc.top, "schema")? != SCHEMA || string(&doc.top, "issuer")? != issuer {
        return Err(invalid(
            "mapping schema or issuer does not match the requested HTML Claims",
        ));
    }
    if doc.rows.is_empty() || doc.rows.len() > MAX_CLAIMS {
        return Err(invalid("mappings require 1..4096 [[claim]] rows"));
    }
    let mut ids = BTreeSet::new();
    let mut out = Vec::new();
    for row in &doc.rows {
        if row.table != "claim" {
            return Err(invalid(format!("line {}: expected [[claim]]", row.line)));
        }
        only(
            &row.fields,
            &[
                "id",
                "file",
                "table",
                "cell",
                "subject_cell",
                "subject_label",
                "period_cell",
                "period_label",
                "metric_cell",
                "metric_label",
                "unit_cell",
                "unit_label",
                "ownership_cell",
                "ownership_label",
            ],
        )?;
        let id = string(&row.fields, "id")?;
        if id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            || !ids.insert(id.clone())
        {
            return Err(invalid(format!(
                "line {}: duplicate or invalid claim id {id:?}",
                row.line
            )));
        }
        let file = string(&row.fields, "file")?;
        if !profile
            .html_sources
            .iter()
            .any(|source| source.file == file)
        {
            return Err(invalid(format!(
                "{file:?} is not a registered [[html]] source"
            )));
        }
        let table = match row.get("table").map(|field| &field.value) {
            Some(TomlValue::Int(index @ 1..=1024)) => *index as usize,
            _ => return Err(invalid("table must be a 1-based integer in 1..1024")),
        };
        let cell = string(&row.fields, "cell")?;
        coordinates(&cell)?;
        let mut labels = Vec::new();
        for role in ROLES {
            let at = string(&row.fields, &format!("{role}_cell"))?;
            coordinates(&at)?;
            labels.push((at, string(&row.fields, &format!("{role}_label"))?));
        }
        out.push(Mapping {
            id,
            file,
            table,
            cell,
            labels,
            line: row.line,
        });
    }
    Ok(out)
}

fn citation<'a>(
    document: &'a HtmlDocument,
    table: usize,
    at: &str,
) -> Result<(HtmlCellCitation, &'a HtmlSourceCell), Refusal> {
    let (row, column) = coordinates(at)?;
    let cell = document
        .tables()
        .get(table - 1)
        .and_then(|grid| grid.cell(row, column))
        .ok_or_else(|| {
            invalid(format!(
                "{}:Table{table}!{at} has no source cell",
                document.artifact_path()
            ))
        })?;
    let anchor = document
        .make_anchor(table - 1, row, column)
        .ok_or_else(|| invalid("source cell cannot produce an original-cell anchor"))?;
    Ok((
        HtmlCellCitation {
            anchor,
            requested_locator: format!("Table{table}!{at}"),
            byte_range: cell.byte_range(),
            text: cell.text().to_string(),
            row_span: cell.row_span(),
            column_span: cell.column_span(),
        },
        cell,
    ))
}

fn ownership(label: &str) -> Result<OwnershipBasis, Refusal> {
    let lower = label.to_ascii_lowercase();
    if matches!(lower.as_str(), "100%" | "100% basis") {
        return Ok(OwnershipBasis::Consolidated100);
    }
    let percent = lower
        .strip_prefix("attributable (")
        .and_then(|text| text.strip_suffix("%)"))
        .ok_or_else(|| invalid(format!("unsupported explicit ownership label {label:?}")))?;
    let (whole, fraction) = percent.split_once('.').unwrap_or((percent, ""));
    if whole.is_empty()
        || whole.len() > 3
        || fraction.len() > 3
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|c| c.is_ascii_digit())
        || (percent.contains('.') && fraction.is_empty())
    {
        return Err(invalid(
            "attributable ownership requires an exact percentage with at most three decimals",
        ));
    }
    let numerator = format!("{whole}{fraction}")
        .parse::<i64>()
        .map_err(|_| invalid("ownership percentage is outside its exact range"))?;
    let denominator = 100 * 10_i64.pow(fraction.len() as u32);
    if numerator <= 0 || numerator > denominator {
        return Err(invalid("attributable ownership must be in (0,100]%"));
    }
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    Ok(OwnershipBasis::Attributable {
        num: numerator / a,
        den: denominator / a,
        source: ShareSource::Printed,
    })
}

fn metric_unit(metric: &str, unit: &str) -> Result<(&'static str, &'static str), Refusal> {
    let metric = metric.to_ascii_lowercase();
    let unit = unit.to_ascii_lowercase();
    let cost = matches!(unit.as_str(), "us$/oz sold" | "usd/oz sold");
    let percent = matches!(unit.as_str(), "%" | "percent");
    Ok(match metric.as_str() {
        "aisc" | "all-in sustaining costs" if cost => ("html:aisc_usd_per_oz_sold", "usd_per_oz"),
        "tcc" | "total cash costs" if cost => ("html:tcc_usd_per_oz_sold", "usd_per_oz"),
        "gold-equivalent production" | "gold equivalent production"
            if matches!(unit.as_str(), "koz" | "thousand gold-equivalent ounces") =>
        {
            ("html:gold_equivalent_production_koz", "koz")
        }
        "processed grade" | "head grade" if matches!(unit.as_str(), "g/t" | "grams per tonne") => {
            ("html:processed_grade_g_per_t", "g_per_t")
        }
        "processed grade" | "head grade" if percent => ("html:processed_grade_percent", "percent"),
        "recovery" | "gold recovery" if percent => ("html:recovery_percent", "percent"),
        _ => {
            return Err(invalid(format!(
                "unsupported metric/unit labels {metric:?} / {unit:?}"
            )));
        }
    })
}

fn typed_value(cell: &HtmlSourceCell, status: Option<StatusKind>) -> Result<PublicValue, Refusal> {
    if let Some(number) = cell.number() {
        let reason = match status {
            Some(StatusKind::Disposed) => Some(NilReason::Disposed),
            Some(StatusKind::Withheld) => Some(NilReason::Withheld),
            _ => None,
        };
        return Ok(match reason {
            Some(reason) if number.value.mantissa == 0 => PublicValue::Nil {
                token: number.raw.clone(),
                reason,
            },
            _ => PublicValue::Number {
                stored: number.raw.clone(),
                displayed: Ok(number.raw.clone()),
                under_status: reason.and(status),
            },
        });
    }
    let token = cell.text();
    Ok(match token.trim() {
        "" => PublicValue::Missing,
        "-" | "–" | "—" => PublicValue::Nil {
            token: token.into(),
            reason: NilReason::Unknown,
        },
        "n/a" | "N/A" => PublicValue::NotApplicable {
            token: token.into(),
        },
        _ => {
            return Err(invalid(format!(
                "data cell contains neither one exact bare number nor a placeholder: {token:?}"
            )));
        }
    })
}

fn project(
    profile: &IssuerProfile,
    source: &HtmlSourceRow,
    document: &HtmlDocument,
    mapping: &Mapping,
    definitions_anchor: &SourceAnchor,
    registry_anchor: &SourceAnchor,
) -> Result<HtmlClaim, Refusal> {
    let mut labels = Vec::new();
    for (role, (at, expected)) in ROLES.into_iter().zip(&mapping.labels) {
        let (found, _) = citation(document, mapping.table, at)?;
        if found.text != *expected {
            return Err(invalid(format!(
                "{} {role} label at {}: expected {expected:?}, found {:?}",
                mapping.id, found.anchor.locator, found.text
            )));
        }
        labels.push(found);
    }
    let subject = &labels[0].text;
    if !profile.subjects.contains(subject)
        || scope_for(profile, subject, &source.release) != Scope::Site
    {
        return Err(invalid(format!(
            "{subject:?} must exactly name a registered site, not a region or company total"
        )));
    }
    let period = parse_period_code(&labels[1].text)
        .or_else(|| parse_sheet_period(&labels[1].text).ok())
        .ok_or_else(|| {
            invalid(format!(
                "unsupported explicit period label {:?}",
                labels[1].text
            ))
        })?;
    let (metric, unit) = metric_unit(&labels[2].text, &labels[3].text)?;
    let basis = ownership(&labels[4].text)?;
    let status = profile
        .subject_statuses
        .iter()
        .find(|status| status.subject == *subject && status.applies(&period));
    let (value_source, source_cell) = citation(document, mapping.table, &mapping.cell)?;
    let value = typed_value(source_cell, status.map(|status| status.kind))?;
    let mut mapping_source = definitions_anchor.clone();
    mapping_source.locator = format!("claim:{}:line:{}", mapping.id, mapping.line);
    let mut context: Vec<_> = labels.iter().map(|label| label.anchor.clone()).collect();
    context.push(mapping_source.clone());
    context.push(registry_anchor.clone());
    let mut cell = public_claim_anchored(
        value_source.anchor.clone(),
        context,
        PublicKey {
            issuer: profile.issuer.clone(),
            subject: subject.clone(),
            valid_period: period,
            reporting_vintage: Vintage {
                release: source.release.clone(),
                file_sha256: source.sha256.clone(),
            },
            ownership_basis: basis,
            scope: Scope::Site,
            unit: unit.into(),
            line: LineKey {
                label: metric.into(),
                occurrence: 0,
            },
        },
        value,
    );
    cell.label_as_printed = Some(labels[2].text.clone());
    cell.basis_evidence = Some(format!(
        "{}:{}; mapping {}#{}",
        source.file, labels[4].anchor.locator, mapping_source.artifact_path, mapping_source.locator
    ));
    cell.unit_evidence = Some(format!(
        "{}:{}; mapping {}#{}",
        source.file, labels[3].anchor.locator, mapping_source.artifact_path, mapping_source.locator
    ));
    cell.status_evidence = status.map(|status| status.evidence.clone());
    let values = [
        subject.clone(),
        period.code(),
        metric.to_string(),
        unit.to_string(),
        format!("{basis:?}"),
    ];
    let mut interpretations: Vec<_> = ROLES
        .into_iter()
        .zip(values)
        .zip(&labels)
        .map(|((role, value), source)| HtmlInterpretation {
            role,
            value,
            source: source.clone(),
        })
        .collect();
    interpretations.push(HtmlInterpretation {
        role: "scope",
        value: "site (registered subject)".into(),
        source: labels[0].clone(),
    });
    Ok(HtmlClaim {
        definition_id: mapping.id.clone(),
        cell,
        value_source,
        interpretations,
        mapping_source,
        exact_number: source_cell.number().cloned(),
    })
}

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, Refusal> {
    let error = |error| crate::input_refusal(format!("cannot read {}: {error}", path.display()));
    let file = std::fs::File::open(path).map_err(error)?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > limit {
        return Err(invalid(format!(
            "{} exceeds its {limit}-byte input bound",
            path.display()
        )));
    }
    Ok(bytes)
}

fn within(root: &Path, path: &Path) -> Result<(PathBuf, String), Refusal> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let resolved = joined
        .canonicalize()
        .map_err(|error| crate::input_refusal(format!("{}: {error}", joined.display())))?;
    let relative = resolved
        .strip_prefix(root)
        .map_err(|_| invalid("source and mapping files must reside in the workspace"))?;
    let name = relative
        .to_str()
        .ok_or_else(|| invalid("source paths must be UTF-8"))?
        .to_string();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(invalid(
            "source paths must be nonempty and contain no controls",
        ));
    }
    Ok((resolved, name))
}

/// Replays declared HTML mappings from registered, hash-verified sources without fetching anything.
/// Publication cuts apply before opening HTML files; a later excluded source never replaces an earlier one.
pub fn run(
    root: &Path,
    issuer: &str,
    definitions: &Path,
    as_of: Option<&str>,
) -> Result<HtmlClaimsReport, Refusal> {
    if !crate::sources::is_issuer_name(issuer) || as_of.is_some_and(|date| !is_date(date)) {
        return Err(invalid(
            "issuer must be a registry name and as_of, when supplied, a calendar date",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|error| crate::input_refusal(format!("workspace: {error}")))?;
    let registry_path = format!("registries/sources/{issuer}.toml");
    let (registry_file, registry_name) = within(&root, Path::new(&registry_path))?;
    let registry_bytes = bounded_read(&registry_file, MAX_DEFINITIONS_BYTES)?;
    let registry_text = std::str::from_utf8(&registry_bytes)
        .map_err(|_| invalid("source registry must be UTF-8"))?;
    let registry = parse_sources_registry(registry_text)
        .map_err(|error| crate::input_refusal(error.to_string()))?;
    if registry.issuer != issuer {
        return Err(invalid(
            "source registry issuer does not match its selector",
        ));
    }
    let profile = IssuerProfile::from_registry(registry).map_err(invalid)?;
    let (definitions_file, definitions_path) = within(&root, definitions)?;
    let definitions_bytes = bounded_read(&definitions_file, MAX_DEFINITIONS_BYTES)?;
    let definitions_text =
        std::str::from_utf8(&definitions_bytes).map_err(|_| invalid("mappings must be UTF-8"))?;
    let mappings = mappings(definitions_text, issuer, &profile)?;
    let definitions_hash = sha256(&definitions_bytes);
    let registry_hash = sha256(&registry_bytes);
    let definitions_anchor = SourceAnchor::new(&definitions_path, definitions_hash.0, "");
    let registry_anchor = SourceAnchor::new(
        registry_name,
        registry_hash.0,
        format!("issuer:{issuer}:html"),
    );
    let mut sources = Vec::new();
    let mut excluded_sources = Vec::new();
    for source in &profile.html_sources {
        if !mappings.iter().any(|mapping| mapping.file == source.file) {
            continue;
        }
        if as_of.is_some_and(|cut| source.published_at.as_str() > cut) {
            excluded_sources.push(source.clone());
        } else {
            sources.push(source.clone());
        }
    }
    let mut claims = Vec::new();
    let (mut origins, mut keys) = (BTreeSet::new(), BTreeSet::new());
    for source in &sources {
        if source.bytes > HTML_MAX_BYTES as u64 {
            return Err(invalid("registered HTML exceeds the reader byte bound"));
        }
        let (file, relative) = within(&root, &Path::new(&source.dir).join(&source.file))?;
        let bytes = bounded_read(&file, HTML_MAX_BYTES)?;
        if bytes.len() as u64 != source.bytes || sha256(&bytes).to_hex() != source.sha256 {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "HTML source {} differs from its registered bytes or SHA-256",
                    source.file
                ),
            ));
        }
        let document = HtmlDocument::from_bytes(relative, &bytes)?;
        for mapping in mappings
            .iter()
            .filter(|mapping| mapping.file == source.file)
        {
            let claim = project(
                &profile,
                source,
                &document,
                mapping,
                &definitions_anchor,
                &registry_anchor,
            )?;
            if !origins.insert((
                source.file.clone(),
                claim.value_source.anchor.locator.clone(),
            )) {
                return Err(invalid(format!(
                    "{} maps one source-cell origin more than once",
                    mapping.id
                )));
            }
            if !keys.insert(claim.cell.key.clone()) {
                return Err(invalid(format!(
                    "{} repeats a semantic key in the same reporting vintage",
                    mapping.id
                )));
            }
            claims.push(claim);
        }
    }
    let claims_digest = claims_digest(claims.iter().map(|claim| &claim.cell));
    Ok(HtmlClaimsReport {
        issuer: issuer.into(),
        definitions_path,
        definitions_sha256: definitions_hash.to_hex(),
        registry_sha256: registry_hash.to_hex(),
        as_of: as_of.map(str::to_string),
        sources,
        excluded_sources,
        claims,
        claims_digest,
    })
}
