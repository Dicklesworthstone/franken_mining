//! DESIGN (BD-346): a public, source-bound production/sales mirror for explicitly selected reporters.
//! The agreed perimeter and separately cited equity share are inputs, never inferred from partner names.
//! Determinism: Exact (canonical source ordering and checked rational rounding/ownership arithmetic).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::Claim;
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Digest32, blake3_hash, sha256};
use fmine_evidence::{EvidenceClass, Refusal, RefusalCode};
use fmine_metrics::guidance::{Ownership, Period, QuantityKind};
use fmine_metrics::mirror::transcriptions::{self, keys, period_label as period};
use fmine_metrics::mirror::{self, MirrorReading, MirrorReport, MirrorVerdict, Perimeter};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub asset: String,
    pub period: Period,
    pub as_of: String,
    pub metric: QuantityKind,
    pub reporters: [ReporterSelection; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Public(Selection),
    Synthetic,
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
        "production" => QuantityKind::ProductionKoz,
        "sold" => QuantityKind::SoldKoz,
        _ => {
            return Err(
                "mirror --metric supports production or sold; no cost bridge is inferred".into(),
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
    let bound = transcriptions::bind(
        &selected.asset,
        selected.period,
        selected.metric,
        issuer,
        &document,
        &rows,
        evidence,
    )?;
    Ok((
        Source {
            issuer: issuer.into(),
            document,
            rows,
            ownership: bound.ownership,
        },
        bound.inputs,
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
        &keys(&selected.asset, selected.period, selected.metric),
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
    report: MirrorReport,
    synthetic: bool,
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
        || !matches!(
            selection.metric,
            QuantityKind::ProductionKoz | QuantityKind::SoldKoz
        )
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
        report: mirror::compare(&inputs)?,
        synthetic: false,
    })
}

fn synthetic() -> Result<Card> {
    let selection = Selection {
        asset: "invented-jv".into(),
        period: Period {
            year: 2030,
            first_month: 1,
            last_month: 12,
        },
        as_of: "2031-02-01".into(),
        metric: QuantityKind::ProductionKoz,
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
    let keys = keys(&selection.asset, selection.period, selection.metric);
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
        for (key, token, mode, unit) in [
            (&keys[0], "1000", "100%", "koz"),
            (
                &keys[1],
                if i == 0 { "615" } else { "385" },
                "attributable",
                "koz",
            ),
            (
                &keys[2],
                if i == 0 { "61.5" } else { "38.5" },
                "equity",
                "percent",
            ),
        ] {
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
                quantity: key.clone(),
                unit: unit.into(),
                basis: format!("{mode}{scope}"),
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
        report: mirror::compare(&inputs)?,
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
        Command::Synthetic => synthetic(),
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
fn reading(c: &Claim<MirrorReading>) -> String {
    let ownership = match c.value.quantity.ownership {
        Ownership::Full => "{\"kind\":\"100%\"}".into(),
        Ownership::Attributable { share } => {
            format!("{{\"kind\":\"attributable\",\"share\":{}}}", rat(share))
        }
    };
    format!(
        "{{{},\"reporter\":{},\"perimeter\":{},\"source\":{},\"ownership\":{},\"interval_koz\":{}}}",
        claim(c),
        j(&c.value.reporter),
        perimeter(&c.value.perimeter),
        source_anchor(&c.value.source),
        ownership,
        interval(c.value.quantity.interval)
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

impl Card {
    fn verdict(&self) -> String {
        match &self.report.verdict {
            MirrorVerdict::Consistent { witness } => format!(
                "{{\"status\":\"consistent\",\"independently_checked\":true,\"witness\":[{}]}}",
                witness
                    .iter()
                    .map(|(cell, value)| format!(
                        "{{\"cell\":{},\"value\":{}}}",
                        j(cell),
                        rat(*value)
                    ))
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
    fn body(&self) -> String {
        let sources = self.sources.iter().map(|s| {
            let d=&s.document;
            let rows=s.rows.iter().map(|r|format!("{{\"quantity\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"locator\":{},\"readers\":[{}]}}", j(&r.quantity),j(&r.token),j(&r.unit),j(&r.basis),j(&r.period),j(&r.locator.to_string()),r.readers.iter().map(|v|j(v)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
            format!("{{\"issuer\":{},\"document\":{{\"file\":{},\"kind\":{},\"url\":{},\"bytes\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{}}},\"readings\":[{rows}],\"ownership_claim\":{}}}",j(&s.issuer),j(&d.file),j(&d.kind),j(&d.url),d.bytes,j(&d.sha256),j(&d.published_at),j(&d.published_at_basis),s.ownership.as_ref().map_or("null".into(),|c|format!("{{{},\"fraction\":{}}}",claim(c),rat(c.value))))
        }).collect::<Vec<_>>().join(",");
        let common = self.report.common.as_ref().map_or("null".into(), |c| {
            format!(
                "{{{},\"perimeter\":{},\"interval_koz\":{}}}",
                claim(c),
                perimeter(&c.value.perimeter),
                interval(c.value.quantity.interval)
            )
        });
        let cells = self
            .report
            .cells
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
            .join(",");
        format!(
            "\"schema\":{},\"asset\":{},\"period\":{},\"as_of\":{},\"metric\":{},\"synthetic\":{},\"gate_eligible\":false,\"jv_sensitive\":true,\"no_affiliation\":{},\"sources\":[{sources}],\"inputs\":[{}],\"normalized\":[{}],\"common\":{common},\"verdict\":{},\"proof\":{{\"rounding\":\"closed_half_last_place\",\"physical_nonnegative\":{},\"cells\":[{cells}],\"identities\":[{}]}},\"no_claim\":{}",
            j(SCHEMA),
            j(&self.selection.asset),
            j(&period(self.selection.period)),
            j(&self.selection.as_of),
            j(if self.selection.metric == QuantityKind::ProductionKoz {
                "gold_produced_koz"
            } else {
                "gold_sold_koz"
            }),
            self.synthetic,
            j(if self.synthetic {
                "Invented synthetic evidence; no issuer affiliation."
            } else {
                "From public filings; no affiliation with either issuer implied."
            }),
            self.report
                .inputs
                .iter()
                .map(reading)
                .collect::<Vec<_>>()
                .join(","),
            self.report
                .normalized
                .iter()
                .map(reading)
                .collect::<Vec<_>>()
                .join(","),
            self.verdict(),
            self.report.physical_nonnegative,
            self.report
                .identities
                .iter()
                .map(identity)
                .collect::<Vec<_>>()
                .join(","),
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
        let clean = |s: &str| {
            s.replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace('`', "\\`")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace(['\r', '\n'], " ")
        };
        let status = match self.report.verdict {
            MirrorVerdict::Consistent { .. } => "Consistent",
            MirrorVerdict::Conflict { .. } => "Conflict",
            MirrorVerdict::Indeterminate { .. } => "Indeterminate",
        };
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
        if let Some(common) = &self.report.common {
            let v = common.value.quantity.interval;
            let _ = write!(
                out,
                "\nCommon 100% interval: [{}/{}, {}/{}] koz. This is a compatibility witness, not a measurement of physical truth.\n",
                v.lo.num(),
                v.lo.den(),
                v.hi.num(),
                v.hi.den()
            );
        }
        if let MirrorVerdict::Conflict {
            certificate_digest,
            minimal_source_sets,
            blame,
            ..
        } = &self.report.verdict
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
                                    self.report.inputs[*i].value.reporter,
                                    self.report.inputs[*i].value.source.locator
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
        if let MirrorVerdict::Indeterminate { reason } = &self.report.verdict {
            let _ = write!(out, "\nUnresolved: {}.\n", clean(reason));
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
