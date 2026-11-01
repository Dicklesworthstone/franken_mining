//! DESIGN (BD-346): source-bound production, sales and capex mirrors for explicitly selected reporters.
//! The agreed perimeter and separately cited equity share are inputs, never inferred from partner names.
//! Determinism: Exact (canonical source ordering and checked rational rounding/ownership arithmetic).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::Claim;
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Digest32, blake3_hash, sha256};
use fmine_evidence::{EvidenceClass, Refusal, RefusalCode};
use fmine_metrics::guidance::{Ownership, Period};
use fmine_metrics::mirror::transcriptions::{self, keys, period_label as period};
use fmine_metrics::mirror::{
    self, CapexBasis, CapexReport, CapexScope, MirrorMetric, MirrorReading, MirrorReport,
    MirrorVerdict, Perimeter,
};
use fmine_numerics::exact_feas::{BlameOutcome, Certificate, Identity, IdentityKind, Rat};
use fmine_public::sources::{DocumentRow, is_issuer_name, load_issuer_profile};
use fmine_public::transcribe::load_transcription_quantities_as_of;
use fmine_registry::sources::is_date;
use fmine_registry::transcriptions::{Accepted, Locator, parse_token};
use fmine_time::{CivilDate, days_in_month};

use crate::guidance_capacity::{interval, rational as rat};
use crate::{Envelope, NextAction};

pub const NO_CLAIM: &str = "Conditional compatibility of selected public assertions under their stated ownership and common dated perimeter. Agreement does not establish physical truth or select a correct reporter. No cost bridge, valuation, disclosure permission or partner affiliation is implied.";
const SCHEMA: &str = "fmine.jv_mirror.v1";
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReporterSelection {
    pub issuer: String,
    pub document: String,
}

/// One quantity, or all three capital scopes with a separately declared joint footing check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricSelection {
    Single(MirrorMetric),
    Capex,
}

impl MetricSelection {
    fn kinds(self) -> Vec<MirrorMetric> {
        match self {
            Self::Single(kind) => vec![kind],
            Self::Capex => vec![
                MirrorMetric::Capex(CapexScope::Sustaining),
                MirrorMetric::Capex(CapexScope::Project),
                MirrorMetric::Capex(CapexScope::Total),
            ],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Single(kind) => metric_name(kind),
            Self::Capex => "capex_usd_m",
        }
    }

    fn keys(self, asset: &str, period: Period) -> Vec<String> {
        let mut selected: Vec<_> = self
            .kinds()
            .into_iter()
            .flat_map(|kind| keys(asset, period, kind))
            .collect();
        selected.sort();
        selected.dedup();
        selected
    }
}

fn metric_name(kind: MirrorMetric) -> &'static str {
    match kind {
        MirrorMetric::ProductionKoz => "gold_produced_koz",
        MirrorMetric::SoldKoz => "gold_sold_koz",
        MirrorMetric::Capex(CapexScope::Sustaining) => "capex_sustaining_usd_m",
        MirrorMetric::Capex(CapexScope::Project) => "capex_project_usd_m",
        MirrorMetric::Capex(CapexScope::Total) => "capex_total_usd_m",
    }
}

fn unit(kind: MirrorMetric) -> &'static str {
    match kind {
        MirrorMetric::Capex(_) => "USD_m",
        MirrorMetric::ProductionKoz | MirrorMetric::SoldKoz => "koz",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub asset: String,
    pub period: Period,
    pub as_of: String,
    pub metric: MetricSelection,
    pub reporters: [ReporterSelection; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Public(Selection),
    Synthetic,
    SyntheticCapex,
}

fn fail(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("JV mirror: {}", reason.into()))
}

fn slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

fn parse_period(s: &str) -> Option<Period> {
    let year = s.get(..4)?.parse::<u16>().ok().filter(|y| *y >= 1900)?;
    let (first_month, last_month) = match s.get(4..)? {
        "" | "FY" => (1, 12),
        "Q1" => (1, 3),
        "Q2" => (4, 6),
        "Q3" => (7, 9),
        "Q4" => (10, 12),
        "H1" => (1, 6),
        "H2" => (7, 12),
        _ => return None,
    };
    Some(Period {
        year,
        first_month,
        last_month,
    })
}

fn date_text(d: CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

/// Common CLI options are removed by the main router; all remaining selectors are explicit and unique.
pub fn parse(args: &[String]) -> std::result::Result<Command, String> {
    if args == ["mirror", "--synthetic"] {
        return Ok(Command::Synthetic);
    }
    if args == ["mirror", "--synthetic", "--metric", "capex"]
        || args == ["mirror", "--metric", "capex", "--synthetic"]
    {
        return Ok(Command::SyntheticCapex);
    }
    let mut values = BTreeMap::new();
    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        if !matches!(
            flag,
            "--asset"
                | "--period"
                | "--as-of"
                | "--metric"
                | "--left-issuer"
                | "--left-document"
                | "--right-issuer"
                | "--right-document"
        ) {
            return Err(format!(
                "mirror: unsupported option {flag}; --synthetic is exclusive with source selectors"
            ));
        }
        let value = args
            .get(i + 1)
            .filter(|v| !v.is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("mirror: missing value for {flag}"))?;
        if values.insert(flag, value.as_str()).is_some() {
            return Err(format!("mirror: duplicate {flag}"));
        }
        i += 2;
    }
    let required = |key| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("mirror requires {key}"))
    };
    let asset = required("--asset")?;
    if !slug(asset) {
        return Err("mirror --asset needs a lowercase hyphenated key".into());
    }
    let period = parse_period(required("--period")?)
        .ok_or("mirror --period needs YYYY, YYYYQn or YYYYHn")?;
    let as_of = required("--as-of")?;
    if !is_date(as_of) {
        return Err("mirror --as-of needs YYYY-MM-DD".into());
    }
    let metric = match values.get("--metric").copied().unwrap_or("production") {
        "production" => MetricSelection::Single(MirrorMetric::ProductionKoz),
        "sold" => MetricSelection::Single(MirrorMetric::SoldKoz),
        "capex-sustaining" => MetricSelection::Single(MirrorMetric::Capex(CapexScope::Sustaining)),
        "capex-project" => MetricSelection::Single(MirrorMetric::Capex(CapexScope::Project)),
        "capex-total" => MetricSelection::Single(MirrorMetric::Capex(CapexScope::Total)),
        "capex" => MetricSelection::Capex,
        _ => {
            return Err(
                "mirror --metric supports production, sold, capex-sustaining, capex-project, capex-total or capex".into(),
            );
        }
    };
    let reporters = [
        ReporterSelection {
            issuer: required("--left-issuer")?.into(),
            document: required("--left-document")?.into(),
        },
        ReporterSelection {
            issuer: required("--right-issuer")?.into(),
            document: required("--right-document")?.into(),
        },
    ];
    if reporters
        .iter()
        .any(|s| !is_issuer_name(&s.issuer) || s.issuer == "synthetic")
        || reporters[0].issuer == reporters[1].issuer
    {
        return Err(
            "mirror needs two distinct registered issuer names; invented evidence uses --synthetic"
                .into(),
        );
    }
    Ok(Command::Public(Selection {
        asset: asset.into(),
        period,
        as_of: as_of.into(),
        metric,
        reporters,
    }))
}

#[derive(Debug, Clone)]
struct Source {
    issuer: String,
    document: DocumentRow,
    rows: Vec<Accepted>,
    ownership: Option<Claim<Rat>>,
}

fn bind_rows(
    selected: &Selection,
    issuer: &str,
    document: DocumentRow,
    rows: Vec<Accepted>,
    evidence: EvidenceClass,
) -> Result<(Source, Vec<Claim<MirrorReading>>)> {
    let mut ownership = None;
    let mut inputs = Vec::new();
    for kind in selected.metric.kinds() {
        let bound = transcriptions::bind(
            &selected.asset,
            selected.period,
            kind,
            issuer,
            &document,
            &rows,
            evidence,
        )?;
        if let Some(share) = bound.ownership {
            if ownership
                .as_ref()
                .is_some_and(|prior: &Claim<Rat>| prior.claim_id != share.claim_id)
            {
                return Err(fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    "capex scopes do not share the same cited ownership assertion",
                ));
            }
            ownership = Some(share);
        }
        inputs.extend(bound.inputs);
    }
    Ok((
        Source {
            issuer: issuer.into(),
            document,
            rows,
            ownership,
        },
        inputs,
    ))
}

fn load_side(
    root: &Path,
    selected: &Selection,
    reporter: &ReporterSelection,
) -> Result<(Source, Vec<Claim<MirrorReading>>)> {
    let profile = load_issuer_profile(root, &reporter.issuer)?;
    let document = profile
        .documents
        .iter()
        .find(|d| d.file == reporter.document)
        .cloned()
        .ok_or_else(|| {
            fail(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "{}/{} is not a registered document",
                    reporter.issuer, reporter.document
                ),
            )
        })?;
    if !matches!(document.kind.as_str(), "mda" | "aif" | "partner_filing")
        || !document.published_at_basis.starts_with("FACT: ")
    {
        return Err(fail(
            RefusalCode::NonIdentifiableAttribution104,
            "actuals need a registered filing with an explicitly evidenced publication date",
        ));
    }
    let end = format!(
        "{:04}-{:02}-{:02}",
        selected.period.year,
        selected.period.last_month,
        days_in_month(i32::from(selected.period.year), selected.period.last_month)
    );
    if document.published_at > selected.as_of {
        return Err(fail(
            RefusalCode::AssayLookahead108,
            format!("{} is later than the as-of cut", document.file),
        ));
    }
    if document.published_at < end {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "a complete-period actual cannot be published before its period ends",
        ));
    }
    let report = load_transcription_quantities_as_of(
        &profile,
        root,
        &selected.as_of,
        &selected.metric.keys(&selected.asset, selected.period),
        Some(&document.file),
    )?
    .ok_or_else(|| {
        fail(
            RefusalCode::NoClaimInDomain201,
            format!("{} has no local transcription registry", reporter.issuer),
        )
    })?;
    if !report.agreement.is_consistent() || !report.agreement.single_reader.is_empty() {
        return Err(fail(
            RefusalCode::NotObservable202,
            "every selected locator needs complete independent reader agreement",
        ));
    }
    bind_rows(
        selected,
        &reporter.issuer,
        document,
        report.agreement.accepted,
        EvidenceClass::ISSUER,
    )
}

struct Card {
    selection: Selection,
    sources: Vec<Source>,
    report: Report,
    synthetic: bool,
}

enum Report {
    Single(Box<MirrorReport>),
    Capex(Box<CapexReport>),
}

fn public(root: &Path, selection: &Selection) -> Result<Card> {
    if !slug(&selection.asset)
        || parse_period(&period(selection.period)) != Some(selection.period)
        || !is_date(&selection.as_of)
        || selection
            .reporters
            .iter()
            .any(|r| !is_issuer_name(&r.issuer) || r.issuer == "synthetic")
        || selection.reporters[0].issuer == selection.reporters[1].issuer
    {
        return Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "invalid asset, period, evidence cut, metric or reporter selection",
        ));
    }
    let mut sources = Vec::new();
    let mut inputs = Vec::new();
    for reporter in &selection.reporters {
        let (source, rows) = load_side(root, selection, reporter)?;
        sources.push(source);
        inputs.extend(rows);
    }
    sources.sort_by(|a, b| a.issuer.cmp(&b.issuer));
    Ok(Card {
        selection: selection.clone(),
        sources,
        report: match selection.metric {
            MetricSelection::Single(_) => Report::Single(Box::new(mirror::compare(&inputs)?)),
            MetricSelection::Capex => Report::Capex(Box::new(mirror::compare_capex(&inputs)?)),
        },
        synthetic: false,
    })
}

fn synthetic(metric: MetricSelection) -> Result<Card> {
    let selection = Selection {
        asset: "invented-jv".into(),
        period: Period {
            year: 2030,
            first_month: 1,
            last_month: 12,
        },
        as_of: "2031-02-01".into(),
        metric,
        reporters: [
            ReporterSelection {
                issuer: "invented-alpha".into(),
                document: "Alpha.pdf".into(),
            },
            ReporterSelection {
                issuer: "invented-beta".into(),
                document: "Beta.pdf".into(),
            },
        ],
    };
    let mut sources = Vec::new();
    let mut inputs = Vec::new();
    let scope = ";perimeter=invented-jv;members=invented-mine;valid=2030-01-01/2030-12-31;scope_page=2;scope_table=Invented scope;scope_row=Operations;scope_col=2030";
    for (i, reporter) in selection.reporters.iter().enumerate() {
        let bytes = format!(
            "SYNTHETIC invented public mirror evidence for {}",
            reporter.issuer
        );
        let document = DocumentRow {
            file: reporter.document.clone(),
            dir: "synthetic".into(),
            kind: "partner_filing".into(),
            url: format!("https://example.invalid/{}", reporter.document),
            url_basis: "W1 table".into(),
            published_at: "2031-01-15".into(),
            published_at_basis: "FACT: invented fixture date".into(),
            pages: 2,
            bytes: bytes.len() as u64,
            sha256: sha256(bytes.as_bytes()).to_hex(),
        };
        let mut rows = Vec::new();
        let mut declarations = Vec::new();
        for kind in selection.metric.kinds() {
            let keys = keys(&selection.asset, selection.period, kind);
            let (full, attr) = match kind {
                MirrorMetric::Capex(CapexScope::Sustaining) => {
                    ("600", if i == 0 { "369" } else { "231" })
                }
                MirrorMetric::Capex(CapexScope::Project) => {
                    ("400", if i == 0 { "246" } else { "154" })
                }
                _ => ("1000", if i == 0 { "615" } else { "385" }),
            };
            let definition = if matches!(kind, MirrorMetric::Capex(_)) {
                ";timing=cash;leases=excluded;definition_page=2;definition_table=Invented capital policy;definition_row=Cash capital excluding leases;definition_col=2030"
            } else {
                ""
            };
            declarations.push((keys[0].clone(), full, "100%", unit(kind), definition));
            declarations.push((
                keys[1].clone(),
                attr,
                "attributable",
                unit(kind),
                definition,
            ));
            if declarations.iter().all(|(key, ..)| key != &keys[2]) {
                declarations.push((
                    keys[2].clone(),
                    if i == 0 { "61.5" } else { "38.5" },
                    "equity",
                    "percent",
                    "",
                ));
            }
        }
        for (key, token, mode, unit, definition) in declarations {
            rows.push(Accepted {
                locator: Locator {
                    doc: document.file.clone(),
                    page: 1,
                    table: "Invented source".into(),
                    row: key.clone(),
                    col: "2030FY".into(),
                },
                doc_sha256: document.sha256.clone(),
                token: token.into(),
                value: parse_token(token)
                    .map_err(|e| fail(RefusalCode::NumericalCertificateFailed207, e))?,
                quantity: key,
                unit: unit.into(),
                basis: format!("{mode}{scope}{definition}"),
                period: "2030FY".into(),
                readers: vec!["synthetic-a".into(), "synthetic-b".into()],
            });
        }
        let (source, claims) = bind_rows(
            &selection,
            &reporter.issuer,
            document,
            rows,
            EvidenceClass::Assumed,
        )?;
        sources.push(source);
        inputs.extend(claims);
    }
    Ok(Card {
        selection,
        sources,
        report: match metric {
            MetricSelection::Single(_) => Report::Single(Box::new(mirror::compare(&inputs)?)),
            MetricSelection::Capex => Report::Capex(Box::new(mirror::compare_capex(&inputs)?)),
        },
        synthetic: true,
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "Inspect the registered public mirror command.",
    )];
    let result = match command {
        Command::Public(s) => public(root, s),
        Command::Synthetic => synthetic(MetricSelection::Single(MirrorMetric::ProductionKoz)),
        Command::SyntheticCapex => synthetic(MetricSelection::Capex),
    };
    match result {
        Ok(card) => {
            let mut env = Envelope::ok("mirror", card.json(), actions);
            env.no_claim = Some(NO_CLAIM);
            env.rendered_markdown = Some(card.markdown());
            env
        }
        Err(refusal) => Envelope::refused("mirror", refusal, actions),
    }
}

fn j(s: &str) -> String {
    format!("\"{}\"", crate::escape_json(s))
}
fn source_anchor(a: &SourceAnchor) -> String {
    format!(
        "{{\"artifact\":{},\"sha256\":{},\"locator\":{}}}",
        j(&a.artifact_path),
        j(&Digest32(a.artifact_sha256).to_hex()),
        j(&a.locator)
    )
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}
fn claim<T>(c: &Claim<T>) -> String {
    format!("\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"evidence\":{},\"knowledge\":{},\"parents\":[{}],\"anchors\":[{}],\"uu_terms\":[{}]",
        j(&c.claim_id.to_string()), j(&c.metric_key), j(c.value_tag()), j(&hex(&c.canonical_body())), j(&format!("{:?}", c.evidence)), j(&format!("{:?}", c.state)),
        c.parents.iter().map(|p| j(&p.to_string())).collect::<Vec<_>>().join(","), c.anchors.iter().map(source_anchor).collect::<Vec<_>>().join(","),
        c.uu_terms.iter().map(|u| format!("{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}", j(&u.scenario_id), j(&u.description), if u.downside_exposure_usd_m.is_finite() { u.downside_exposure_usd_m.to_string() } else { "null".into() }, j(&u.trigger_indicator))).collect::<Vec<_>>().join(","))
}
fn perimeter(p: &Perimeter) -> String {
    format!(
        "{{\"id\":{},\"operations\":[{}],\"valid_from\":{},\"valid_through\":{}}}",
        j(&p.id),
        p.operations
            .iter()
            .map(|s| j(s))
            .collect::<Vec<_>>()
            .join(","),
        j(&date_text(p.valid_from)),
        j(&date_text(p.valid_through))
    )
}

fn capex_basis(basis: &CapexBasis) -> String {
    format!(
        "{{\"scope\":{},\"timing\":{},\"leases\":{}}}",
        j(&format!("{:?}", basis.scope).to_ascii_lowercase()),
        j(&format!("{:?}", basis.timing).to_ascii_lowercase()),
        j(&format!("{:?}", basis.leases).to_ascii_lowercase())
    )
}

fn amount(kind: MirrorMetric, value: fmine_numerics::exact_feas::Interval) -> String {
    match kind {
        MirrorMetric::ProductionKoz | MirrorMetric::SoldKoz => {
            format!("\"interval_koz\":{}", interval(value))
        }
        MirrorMetric::Capex(_) => format!("\"interval_usd_m\":{}", interval(value)),
    }
}

fn reading(c: &Claim<MirrorReading>) -> String {
    let ownership = match c.value.quantity.ownership {
        Ownership::Full => "{\"kind\":\"100%\"}".into(),
        Ownership::Attributable { share } => {
            format!("{{\"kind\":\"attributable\",\"share\":{}}}", rat(share))
        }
    };
    let capex = match &c.value.quantity.capex_basis {
        Some(basis) => format!(
            ",\"capex_basis\":{},\"basis_source\":{}",
            capex_basis(basis),
            c.value
                .basis_source
                .as_ref()
                .map_or("null".into(), source_anchor)
        ),
        None => String::new(),
    };
    format!(
        "{{{},\"reporter\":{},\"perimeter\":{},\"source\":{},\"ownership\":{},{amount}{capex}}}",
        claim(c),
        j(&c.value.reporter),
        perimeter(&c.value.perimeter),
        source_anchor(&c.value.source),
        ownership,
        amount = amount(c.value.quantity.kind, c.value.quantity.interval)
    )
}
fn proof(c: &Certificate) -> String {
    match c {
        Certificate::Farkas { multipliers } => format!(
            "{{\"kind\":\"farkas\",\"multipliers\":[{}]}}",
            multipliers
                .iter()
                .map(|(id, r)| format!("{{\"constraint\":{},\"multiplier\":{}}}", j(id), rat(*r)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Certificate::Residual { identity, residual } => format!(
            "{{\"kind\":\"residual\",\"identity\":{},\"residual\":{}}}",
            j(identity),
            rat(*residual)
        ),
    }
}
fn blame_status(blame: &BlameOutcome) -> String {
    match blame {
        BlameOutcome::Blamed(_) => {
            "{\"status\":\"checked\",\"minimal_sets_exhaustive\":false}".into()
        }
        BlameOutcome::Indeterminate { reason } => {
            format!("{{\"status\":\"indeterminate\",\"reason\":{}}}", j(reason))
        }
    }
}
fn identity(i: &Identity) -> String {
    let kind = match &i.kind {
        IdentityKind::Scale {
            target,
            source,
            factor,
        } => format!(
            "\"kind\":\"scale\",\"target\":{},\"source\":{},\"factor\":{}",
            j(target),
            j(source),
            rat(*factor)
        ),
        IdentityKind::Equal { a, b } => format!("\"kind\":\"equal\",\"a\":{},\"b\":{}", j(a), j(b)),
        IdentityKind::Sum {
            total,
            lines,
            signs,
        } => format!(
            "\"kind\":\"sum\",\"total\":{},\"lines\":[{}],\"signs\":[{}]",
            j(total),
            lines.iter().map(|s| j(s)).collect::<Vec<_>>().join(","),
            signs
                .iter()
                .map(i8::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        IdentityKind::Ratio {
            rate,
            num,
            den,
            factor,
        } => format!(
            "\"kind\":\"ratio\",\"rate\":{},\"num\":{},\"den\":{},\"factor\":{}",
            j(rate),
            j(num),
            j(den),
            rat(*factor)
        ),
        IdentityKind::WeightedMean { rate, parts } => format!(
            "\"kind\":\"weighted_mean\",\"rate\":{},\"parts\":[{}]",
            j(rate),
            parts
                .iter()
                .map(|(r, w)| format!("[{},{}]", j(r), j(w)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    };
    format!(
        "{{\"id\":{},\"family\":{},\"closure\":{},{kind}}}",
        j(&i.id),
        j(&i.family),
        j(&format!("{:?}", i.closure))
    )
}

fn verdict(value: &MirrorVerdict) -> String {
    match value {
        MirrorVerdict::Consistent { witness } => format!(
            "{{\"status\":\"consistent\",\"independently_checked\":true,\"witness\":[{}]}}",
            witness
                .iter()
                .map(|(cell, value)| format!("{{\"cell\":{},\"value\":{}}}", j(cell), rat(*value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        MirrorVerdict::Conflict {
            certificate,
            certificate_digest,
            minimal_source_sets,
            blame,
        } => format!(
            "{{\"status\":\"conflict\",\"independently_checked\":true,\"certificate_digest\":{},\"certificate\":{},\"blame\":{},\"minimal_source_sets\":[{}]}}",
            j(certificate_digest),
            proof(certificate),
            blame_status(blame),
            minimal_source_sets
                .iter()
                .map(|set| format!(
                    "[{}]",
                    set.iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        MirrorVerdict::Indeterminate { reason } => {
            format!("{{\"status\":\"indeterminate\",\"reason\":{}}}", j(reason))
        }
    }
}

fn proof_cells(cells: &[fmine_numerics::exact_feas::Cell]) -> String {
    cells
        .iter()
        .map(|c| {
            format!(
                "{{\"locator\":{},\"sha256\":{},\"token\":{},\"printed\":{},\"step\":{},\"scope\":{}}}",
                j(&c.locator),
                j(&c.sha256),
                j(&c.token),
                rat(c.printed),
                rat(c.step),
                j(&format!("{:?}", c.scope))
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn single_report(report: &MirrorReport) -> String {
    let common = report.common.as_ref().map_or("null".into(), |c| {
        let basis = c
            .value
            .quantity
            .capex_basis
            .as_ref()
            .map_or(String::new(), |b| {
                format!(",\"capex_basis\":{}", capex_basis(b))
            });
        format!(
            "{{{},\"perimeter\":{},{amount}{basis}}}",
            claim(c),
            perimeter(&c.value.perimeter),
            amount = amount(c.value.quantity.kind, c.value.quantity.interval)
        )
    });
    format!(
        "\"inputs\":[{}],\"normalized\":[{}],\"common\":{common},\"verdict\":{},\"proof\":{{\"rounding\":\"closed_half_last_place\",\"physical_nonnegative\":{},\"cells\":[{}],\"identities\":[{}]}}",
        report
            .inputs
            .iter()
            .map(reading)
            .collect::<Vec<_>>()
            .join(","),
        report
            .normalized
            .iter()
            .map(reading)
            .collect::<Vec<_>>()
            .join(","),
        verdict(&report.verdict),
        report.physical_nonnegative,
        proof_cells(&report.cells),
        report
            .identities
            .iter()
            .map(identity)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn capex_report(report: &CapexReport) -> String {
    let scopes = report
        .lines
        .iter()
        .map(|line| {
            let kind = line.inputs.first().map(|input| input.value.quantity.kind);
            format!(
                "{{\"metric\":{},\"construction\":\"independent_half_last_place\",{}}}",
                kind.map_or("null".into(), |kind| j(metric_name(kind))),
                single_report(line)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let joint = report.joint.as_ref().map_or("null".into(), |joint| {
        format!(
            "{{\"construction\":\"component_sum\",\"inputs\":[{}],\"normalized\":[{}],\"verdict\":{},\"proof\":{{\"rounding\":\"closed_half_last_place\",\"physical_nonnegative\":false,\"cells\":[{}],\"identities\":[{}]}}}}",
            joint.inputs.iter().map(reading).collect::<Vec<_>>().join(","),
            joint.normalized.iter().map(reading).collect::<Vec<_>>().join(","),
            verdict(&joint.verdict),
            proof_cells(&joint.cells),
            joint.identities.iter().map(identity).collect::<Vec<_>>().join(",")
        )
    });
    let overall = report.joint.as_ref().map_or_else(
        || {
            format!(
                "{{\"status\":\"unavailable\",\"reason\":{}}}",
                report
                    .joint_unavailable_reason
                    .as_deref()
                    .map_or("null".into(), j)
            )
        },
        |joint| verdict(&joint.verdict),
    );
    format!(
        "\"scope_checks\":[{scopes}],\"joint\":{joint},\"joint_unavailable_reason\":{},\"verdict\":{overall},\"construction\":\"component_sum\"",
        report
            .joint_unavailable_reason
            .as_deref()
            .map_or("null".into(), j)
    )
}

fn clean(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(['\r', '\n'], " ")
}

fn status(verdict: Option<&MirrorVerdict>) -> &'static str {
    match verdict {
        Some(MirrorVerdict::Consistent { .. }) => "Consistent",
        Some(MirrorVerdict::Conflict { .. }) => "Conflict",
        Some(MirrorVerdict::Indeterminate { .. }) => "Indeterminate",
        None => "Unavailable",
    }
}

fn verdict_markdown(out: &mut String, verdict: &MirrorVerdict, inputs: &[Claim<MirrorReading>]) {
    if let MirrorVerdict::Conflict {
        certificate_digest,
        minimal_source_sets,
        blame,
        ..
    } = verdict
    {
        let _ = writeln!(
            out,
            "\nChecked exclusion certificate `{certificate_digest}`."
        );
        match blame {
            BlameOutcome::Blamed(_) => {
                out.push_str(
                    "\nChecked inclusion-minimal conflicting source sets (not exhaustive):\n",
                );
                for set in minimal_source_sets {
                    let refs = set
                        .iter()
                        .map(|i| {
                            format!(
                                "{} — {}",
                                inputs[*i].value.reporter, inputs[*i].value.source.locator
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    let _ = writeln!(out, "\n- {}", clean(&refs));
                }
            }
            BlameOutcome::Indeterminate { reason } => {
                let _ = writeln!(
                    out,
                    "\nConflict attribution remains indeterminate: {}.",
                    clean(reason)
                );
            }
        }
    }
    if let MirrorVerdict::Indeterminate { reason } = verdict {
        let _ = write!(out, "\nUnresolved: {}.\n", clean(reason));
    }
}

fn single_markdown(out: &mut String, report: &MirrorReport) {
    if let Some(common) = &report.common {
        let quantity = &common.value.quantity;
        let v = quantity.interval;
        let _ = write!(
            out,
            "\nCommon 100% interval: [{}/{}, {}/{}] {}. This is a compatibility witness under the stated basis.\n",
            v.lo.num(),
            v.lo.den(),
            v.hi.num(),
            v.hi.den(),
            unit(quantity.kind)
        );
        if let Some(basis) = &quantity.capex_basis {
            let _ = writeln!(
                out,
                "\nCapital basis: {:?}; {:?} timing; leases {:?}. Signed amounts retain their reported sign.",
                basis.scope, basis.timing, basis.leases
            );
        }
    }
    verdict_markdown(out, &report.verdict, &report.inputs);
}

impl Card {
    fn body(&self) -> String {
        let sources = self.sources.iter().map(|s| {
            let d=&s.document;
            let rows=s.rows.iter().map(|r|format!("{{\"quantity\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"locator\":{},\"readers\":[{}]}}", j(&r.quantity),j(&r.token),j(&r.unit),j(&r.basis),j(&r.period),j(&r.locator.to_string()),r.readers.iter().map(|v|j(v)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
            format!("{{\"issuer\":{},\"document\":{{\"file\":{},\"kind\":{},\"url\":{},\"bytes\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{}}},\"readings\":[{rows}],\"ownership_claim\":{}}}",j(&s.issuer),j(&d.file),j(&d.kind),j(&d.url),d.bytes,j(&d.sha256),j(&d.published_at),j(&d.published_at_basis),s.ownership.as_ref().map_or("null".into(),|c|format!("{{{},\"fraction\":{}}}",claim(c),rat(c.value))))
        }).collect::<Vec<_>>().join(",");
        let assessment = match &self.report {
            Report::Single(report) => single_report(report),
            Report::Capex(report) => capex_report(report),
        };
        format!(
            "\"schema\":{},\"asset\":{},\"period\":{},\"as_of\":{},\"metric\":{},\"synthetic\":{},\"gate_eligible\":false,\"jv_sensitive\":true,\"no_affiliation\":{},\"sources\":[{sources}],{assessment},\"no_claim\":{}",
            j(SCHEMA),
            j(&self.selection.asset),
            j(&period(self.selection.period)),
            j(&self.selection.as_of),
            j(self.selection.metric.name()),
            self.synthetic,
            j(if self.synthetic {
                "Invented synthetic evidence; no issuer affiliation."
            } else {
                "From public filings; no affiliation with either issuer implied."
            }),
            j(NO_CLAIM)
        )
    }
    fn json(&self) -> String {
        let body = self.body();
        format!(
            "{{\"payload_digest\":{},{body}}}",
            j(&blake3_hash(body.as_bytes()).to_hex())
        )
    }
    fn markdown(&self) -> String {
        let overall = match &self.report {
            Report::Single(report) => Some(&report.verdict),
            Report::Capex(report) => report.joint.as_ref().map(|joint| &joint.verdict),
        };
        let status = status(overall);
        let mut out = format!(
            "# JV mirror — {}\n\n{}; as of {}. **{status}** under the stated common perimeter and ownership.\n\n{}\n\n| Reporter | Source assertion | Printed value and unit | Basis |\n|---|---|---|---|\n",
            clean(&self.selection.asset),
            period(self.selection.period),
            self.selection.as_of,
            if self.synthetic {
                "Invented synthetic evidence; no issuer affiliation."
            } else {
                "From public filings; no affiliation with either issuer implied."
            }
        );
        for s in &self.sources {
            for r in &s.rows {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} {} | {} |",
                    clean(&s.issuer),
                    clean(&r.locator.to_string()),
                    clean(&r.token),
                    clean(&r.unit),
                    clean(&r.basis)
                );
            }
        }
        match &self.report {
            Report::Single(report) => single_markdown(&mut out, report),
            Report::Capex(report) => {
                out.push_str("\n## Individual scope checks\n\nThese use each printed value's own half-last-place interval. The joint assessment below uses the separately declared component-sum construction.\n");
                for line in &report.lines {
                    let kind = line.inputs.first().map(|input| input.value.quantity.kind);
                    let _ = writeln!(
                        out,
                        "\n### {} — {}\n",
                        kind.map_or("unavailable metric", metric_name),
                        self::status(Some(&line.verdict))
                    );
                    single_markdown(&mut out, line);
                }
                out.push_str("\n## Joint component-sum check\n\nEach displayed total must equal displayed sustaining plus project capital. The joint calculation then derives the total's bounds from its components; it does not impose a second rounding box on that total.\n");
                if let Some(joint) = &report.joint {
                    let _ = writeln!(out, "\n**{}**.\n", self::status(Some(&joint.verdict)));
                    verdict_markdown(&mut out, &joint.verdict, &joint.inputs);
                } else {
                    let _ = writeln!(
                        out,
                        "\nUnavailable: {}.\n",
                        clean(
                            report
                                .joint_unavailable_reason
                                .as_deref()
                                .unwrap_or("no joint assessment")
                        )
                    );
                }
            }
        }
        for s in &self.sources {
            let _ = write!(
                out,
                "\n{}: {} — SHA-256 `{}`; published {} ({}).\n",
                clean(&s.issuer),
                clean(&s.document.file),
                s.document.sha256,
                s.document.published_at,
                clean(&s.document.published_at_basis)
            );
        }
        let _ = write!(
            out,
            "\n{NO_CLAIM}\n\nPayload digest: `{}`. JSON retains the full exact Claims, normalized intervals and checked proof.\n",
            blake3_hash(self.body().as_bytes()).to_hex()
        );
        out
    }
}
