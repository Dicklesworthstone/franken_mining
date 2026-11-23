//! Source resolution for a frozen public forecast record (BD-076). Original releases supply outcomes and
//! fixed benchmark observations by the assessment cut. Guidance itself must precede the forecast cut;
//! the preregistered Q3 and historical observations can follow it, with manifest membership disclosed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_calib::record::scoring::paired::{Availability, BaselineRow, Bounds, OutcomeRow};
use fmine_calib::record::{
    self, BaselineKind, BaselineRule, InputDoc, Outcome, Quarter, Rat, RecordManifest, RecordRules,
    RegisteredDigest, SiteStatus, Target, TargetMetric, format_utc, parse_date, parse_hex32,
    parse_utc,
};
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::key::{
    self, KeyedCell, OwnershipBasis, Period, PeriodKind, PublicValue, Scope, SeriesOwnershipBasis,
};
use fmine_public::mine_stats::{MineStatsCell, MineStatsValue, View, read_mine_stats};
use fmine_public::sources::{
    DocumentRow, IssuerProfile, SourceKind, is_issuer_name, registry_path, transcriptions_path,
};
use fmine_registry::sources::load_sources_registry;
use fmine_registry::transcriptions::{
    Decimal, EntryRow, Token, TranscriptionRegistry, agree, load_transcriptions, parse_token,
};

use crate::json_string as js;

type Result<T> = std::result::Result<T, Refusal>;

pub(super) struct Resolved {
    pub(super) outcomes: Vec<OutcomeRow>,
    pub(super) baselines: Vec<BaselineRow>,
    pub(super) evidence_json: String,
    pub(super) evidence_markdown: String,
    pub(super) synthetic: bool,
    pub(super) outcome_quarter: Quarter,
}

fn refusal(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("forecast record sources: {}", detail.into()))
}

fn invalid(detail: impl Into<String>) -> Refusal {
    refusal(RefusalCode::UndeclaredUnitBasisOrTz107, detail)
}

fn exact<T, E: std::fmt::Display>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|e| refusal(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), js)
}

fn rat(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}

fn unavailable<T>(reason: impl Into<String>) -> Availability<T> {
    Availability::Unavailable(reason.into())
}

fn number(value: &Availability<Rat>) -> Option<Rat> {
    match value {
        Availability::Available(n) => Some(*n),
        Availability::Unavailable(_) => None,
    }
}

fn reason<T>(value: &Availability<T>) -> Option<&str> {
    match value {
        Availability::Available(_) => None,
        Availability::Unavailable(r) => Some(r),
    }
}

fn basis(basis: OwnershipBasis) -> String {
    match basis {
        OwnershipBasis::Consolidated100 => "100%".into(),
        OwnershipBasis::Attributable { num, den, .. } => format!("attributable:{num}/{den}"),
        OwnershipBasis::Unprinted => "unprinted".into(),
    }
}

fn target_unit(target: &Target) -> Result<&'static str> {
    TargetMetric::from_target(target)
        .map(TargetMetric::unit)
        .ok_or_else(|| {
            invalid(format!(
                "unsupported target field/outcome combination for {}",
                target.id
            ))
        })
}

fn after_quarter(quarter: Quarter) -> Result<i64> {
    quarter
        .end_utc()
        .ok_or_else(|| invalid("quarter completion is outside the UTC calendar"))
}

/// Accept an exact UTC instant or an explicit retrieval interval, including a same-day HH:MM:SSZ end.
/// A bare date is conservative through the end of that day; a quarter end is never a publication time.
fn retrieval_upper(text: &str) -> Result<i64> {
    let mut latest = None;
    let mut day: Option<String> = None;
    for part in text.split('/') {
        let full = if part.len() == 9 && part.ends_with('Z') {
            format!(
                "{}T{part}",
                day.as_deref()
                    .ok_or_else(|| invalid("retrieval interval lacks a date"))?
            )
        } else {
            part.to_string()
        };
        let time = parse_utc(&full)
            .or_else(|| parse_date(&full).and_then(|d| d.checked_mul(86400)?.checked_add(86399)))
            .ok_or_else(|| invalid(format!("unparsed workbook retrieval evidence {text:?}")))?;
        if let Some(prefix) = full.get(..10) {
            day = Some(prefix.to_string());
        }
        if latest.is_some_and(|previous| time < previous) {
            return Err(invalid("retrieval interval is reversed"));
        }
        latest = Some(time);
    }
    latest.ok_or_else(|| invalid("workbook registry lacks retrieval evidence"))
}

fn manifest_input<'a>(manifest: &'a RecordManifest, file: &str) -> Option<&'a InputDoc> {
    manifest.inputs.iter().find(|input| input.doc_id == file)
}

fn frozen_reason(manifest: &RecordManifest, file: &str) -> Option<String> {
    match manifest_input(manifest, file) {
        None => Some(format!("{file} is absent from the frozen input manifest")),
        Some(input)
            if input.retrieved_utc > manifest.information_cutoff_utc
                || input.publication_day > manifest.information_cutoff_utc.div_euclid(86400) =>
        {
            Some(format!(
                "{file} was not available at the frozen information cut"
            ))
        }
        Some(_) => None,
    }
}

fn check_manifest_registry(
    profile: &IssuerProfile,
    manifest: &RecordManifest,
    rules: &RecordRules,
) -> Result<()> {
    let registered = profile
        .sources
        .iter()
        .map(|s| (s.file_name.as_str(), s.sha256_hex.as_str()))
        .chain(
            profile
                .documents
                .iter()
                .map(|d| (d.file.as_str(), d.sha256.as_str())),
        )
        .map(|(file, hash)| {
            parse_hex32(hash)
                .map(|sha256| RegisteredDigest { file, sha256 })
                .ok_or_else(|| {
                    invalid(format!(
                        "registered document {file} lacks a canonical SHA-256"
                    ))
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let findings = record::validate(manifest, &registered, None, rules.dates);
    if !findings.is_empty() {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            findings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    for input in &manifest.inputs {
        if let Some(source) = profile.sources.iter().find(|s| s.file_name == input.doc_id) {
            let quarter = Quarter::parse(&source.quarter)
                .ok_or_else(|| invalid("registered workbook has an invalid release quarter"))?;
            let earliest = after_quarter(quarter)?;
            if input.retrieved_utc < earliest || input.publication_day < earliest.div_euclid(86400)
            {
                return Err(refusal(
                    RefusalCode::AssayLookahead108,
                    format!(
                        "{} claims completed {} data before the quarter finished",
                        input.doc_id, quarter
                    ),
                ));
            }
        }
        let expected = profile
            .sources
            .iter()
            .find(|s| s.file_name == input.doc_id)
            .map(|s| s.bytes)
            .or_else(|| {
                profile
                    .documents
                    .iter()
                    .find(|d| d.file == input.doc_id)
                    .map(|d| d.bytes)
            });
        if expected.is_some_and(|n| n != input.bytes) {
            return Err(refusal(
                RefusalCode::DigestMismatch505,
                format!(
                    "{} byte length differs from its frozen manifest",
                    input.doc_id
                ),
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct Evidence {
    sources: Vec<String>,
    cells: BTreeMap<String, String>,
    bindings: Vec<String>,
    pools: Vec<String>,
    documents: BTreeMap<String, String>,
    markdown: BTreeMap<String, String>,
    notes: Vec<String>,
}

#[derive(Clone)]
struct Release {
    benchmark_reason: Option<String>,
    outcome_reason: Option<String>,
    unreadable: Option<String>,
}

struct Publication {
    day: String,
    basis: String,
}

impl Publication {
    fn upper(&self) -> Result<i64> {
        parse_date(&self.day)
            .and_then(|day| day.checked_mul(86400)?.checked_add(86399))
            .ok_or_else(|| invalid("registered workbook publication date is outside UTC"))
    }
}

/// Resolve only the registered subject spelling before Claim admission. This preserves raw labels
/// and keeps a malformed competing capex cell in the canonical site's ambiguity count.
fn canonical_raw_subject(profile: &IssuerProfile, raw: &MineStatsValue) -> String {
    profile
        .shares
        .iter()
        .find(|share| share.label == raw.site)
        .map_or_else(|| raw.site.clone(), |share| share.subject.clone())
}

struct Observation {
    release: Quarter,
    canonical_subject: String,
    raw: MineStatsValue,
    keyed: std::result::Result<KeyedCell, String>,
    printed: std::result::Result<Rat, String>,
    file: String,
    hash: String,
}

impl Observation {
    fn locator(&self) -> String {
        format!("{}:{}!{}", self.file, self.raw.sheet, self.raw.cell)
    }
    fn raw_token(&self) -> &str {
        match &self.raw.value {
            MineStatsCell::Number { raw_text, .. } => raw_text,
            MineStatsCell::Nil { token } | MineStatsCell::NotApplicable { token } => token,
            MineStatsCell::Missing => "",
        }
    }
    fn evidence(&self) -> String {
        let cell = self.keyed.as_ref().ok();
        let failure = self
            .keyed
            .as_ref()
            .err()
            .map(String::as_str)
            .or_else(|| self.printed.as_ref().err().map(String::as_str));
        format!(
            "{{\"locator\":{},\"sha256\":{},\"raw_kind\":{},\"raw\":{},\"printed\":{},\"claim_id\":{},\"metric\":{},\"unit\":{},\"basis\":{},\"printed_basis\":{},\"basis_evidence\":{},\"status_evidence\":{},\"unusable\":{}}}",
            js(&self.locator()),
            js(&self.hash),
            js(self.raw.value.kind()),
            js(self.raw_token()),
            self.printed
                .as_ref()
                .map_or_else(|_| "null".into(), |n| rat(*n)),
            cell.map_or_else(|| "null".into(), |c| js(&c.claim.claim_id.to_string())),
            cell.map_or_else(|| "null".into(), |c| js(&c.key.line.label)),
            cell.map_or_else(|| "null".into(), |c| js(&c.key.unit)),
            cell.map_or_else(|| "null".into(), |c| js(&basis(c.key.ownership_basis))),
            cell.map_or_else(|| "null".into(), |c| js(&basis(c.printed_basis))),
            optional(cell.and_then(|c| c.basis_evidence.as_deref())),
            optional(cell.and_then(|c| c.status_evidence.as_deref())),
            optional(failure)
        )
    }
    fn markdown(&self) -> String {
        let cell = self.keyed.as_ref().ok();
        format!(
            "| {} | {} | `{}` | {} | {} |",
            crate::guidance::text(&self.locator()),
            cell.map_or_else(|| "unavailable".into(), |c| c.claim.claim_id.to_string()),
            self.hash,
            crate::guidance::text(self.raw_token()),
            crate::guidance::text(&cell.map_or_else(
                || "unusable source interpretation".into(),
                |c| {
                    format!(
                        "{}; {}; {}",
                        c.key.unit,
                        basis(c.key.ownership_basis),
                        c.status_evidence
                            .as_deref()
                            .unwrap_or("no applicable status evidence")
                    )
                }
            ))
        )
    }
}

struct Lookup {
    value: Availability<Rat>,
    basis: Option<OwnershipBasis>,
    locators: Vec<String>,
}

impl Lookup {
    fn missing(reason: impl Into<String>) -> Self {
        Self {
            value: unavailable(reason),
            basis: None,
            locators: Vec::new(),
        }
    }
}

struct Inventory {
    releases: BTreeMap<Quarter, Release>,
    observations: Vec<Observation>,
    evidence: Evidence,
}

impl Inventory {
    fn load(
        root: &Path,
        profile: &IssuerProfile,
        manifest: &RecordManifest,
        wanted: &BTreeSet<Quarter>,
        publications: &BTreeMap<String, Publication>,
        outcome_quarters: &BTreeSet<Quarter>,
        as_of: i64,
    ) -> Result<Self> {
        let vocabulary = profile
            .mine_stats_labels
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut this = Self {
            releases: BTreeMap::new(),
            observations: Vec::new(),
            evidence: Evidence::default(),
        };
        for quarter in wanted {
            let registered = profile
                .sources
                .iter()
                .filter(|s| s.kind == SourceKind::MineStats && s.quarter == quarter.to_string())
                .collect::<Vec<_>>();
            let [source] = registered.as_slice() else {
                let why = if registered.is_empty() {
                    "original release is not registered"
                } else {
                    "original release has multiple registered files"
                };
                this.releases.insert(
                    *quarter,
                    Release {
                        benchmark_reason: Some(why.into()),
                        outcome_reason: Some(why.into()),
                        unreadable: None,
                    },
                );
                this.evidence.sources.push(format!(
                    "{{\"release\":{},\"unavailable\":{},\"registered_files\":[{}]}}",
                    js(&quarter.to_string()),
                    js(why),
                    registered
                        .iter()
                        .map(|s| format!(
                            "{{\"file\":{},\"sha256\":{}}}",
                            js(&s.file_name),
                            js(&s.sha256_hex)
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
                this.evidence
                    .notes
                    .push(format!("Original release {quarter}: {why}."));
                continue;
            };
            let frozen = frozen_reason(manifest, &source.file_name);
            let input = manifest_input(manifest, &source.file_name);
            let publication = publications.get(&source.file_name);
            let available = if let Some(input) = input {
                Some(input.retrieved_utc)
            } else if let Some(publication) = publication {
                Some(publication.upper()?)
            } else {
                retrieval_upper(source.retrieval(&profile.retrieved_utc)).ok()
            };
            let benchmark = match available {
                Some(at) if at < after_quarter(*quarter)? => Some(format!(
                    "{} availability evidence precedes completion of its declared quarter",
                    source.file_name
                )),
                Some(at) if at <= as_of => None,
                Some(at) => Some(format!(
                    "{} availability evidence {} is after the assessment cut",
                    source.file_name,
                    format_utc(at)
                )),
                None => Some("original release has no parsed availability evidence".into()),
            };
            let outcome = if !outcome_quarters.contains(quarter) {
                Some("historical pool release is not a target outcome release".into())
            } else {
                benchmark.clone()
            };
            let mut release = Release {
                benchmark_reason: benchmark,
                outcome_reason: outcome,
                unreadable: None,
            };
            let mut selected = profile.clone();
            selected.sources = vec![(**source).clone()];
            let loaded = if release.benchmark_reason.is_none() {
                match fmine_public::load_registered_sources(&selected, root) {
                    Ok(sources) => sources.into_iter().next(),
                    Err(error) if error.code == "FMINE-REF-506" => {
                        release.unreadable = Some(error.what);
                        None
                    }
                    Err(error) => return Err(error),
                }
            } else {
                None
            };
            this.evidence.sources.push(format!("{{\"release\":{},\"file\":{},\"sha256\":{},\"bytes_verified\":{},\"availability_utc\":{},\"availability_basis\":{},\"published_at\":{},\"published_at_basis\":{},\"in_forecast_manifest\":{},\"manifest_declared_retrieval_utc\":{},\"frozen_input_unavailable\":{},\"benchmark_unavailable\":{},\"outcome_unavailable\":{},\"unreadable\":{}}}",
                js(&source.quarter), js(&source.file_name), js(&source.sha256_hex), loaded.is_some(), available.map_or_else(|| "null".into(), |at| js(&format_utc(at))),
                js(if input.is_some() { "frozen_manifest_retrieval" } else if publication.is_some() { "registered_publication_date_end_of_day_bound" } else { "registry_retrieval_upper_bound_not_publication_date" }),
                optional(publication.map(|p| p.day.as_str())), optional(publication.map(|p| p.basis.as_str())),
                input.is_some(), input.map_or_else(|| "null".into(), |i| js(&format_utc(i.retrieved_utc))), optional(frozen.as_deref()), optional(release.benchmark_reason.as_deref()), optional(release.outcome_reason.as_deref()), optional(release.unreadable.as_deref())));
            if let Some(source) = loaded {
                let read = read_mine_stats(
                    &source.workbook,
                    &source.quarter,
                    (!vocabulary.is_empty()).then_some(&vocabulary),
                )?;
                for error in &read.refusals {
                    let relative = error
                        .locator
                        .strip_prefix(&format!("{}:", source.quarter))
                        .unwrap_or(&error.locator);
                    let locator = format!("{}:{relative}", source.file_name);
                    let raw = relative
                        .split_once('!')
                        .and_then(|(sheet, cell)| {
                            source
                                .workbook
                                .sheets
                                .iter()
                                .find(|s| s.name == sheet)
                                .and_then(|s| {
                                    s.cells_by_coord.values().find(|c| c.cell_ref == cell)
                                })
                        })
                        .and_then(|cell| {
                            cell.number_text.as_deref().or(match &cell.value {
                                fmine_io::xlsx::CellValue::Placeholder(s)
                                | fmine_io::xlsx::CellValue::Text(s)
                                | fmine_io::xlsx::CellValue::FormulaError(s) => Some(s.as_str()),
                                fmine_io::xlsx::CellValue::Number(_) => None,
                            })
                        });
                    this.evidence.bindings.push(format!("{{\"role\":\"reader_refusal\",\"locator\":{},\"sha256\":{},\"raw\":{},\"claim_id\":null,\"reason\":{}}}", js(&locator), js(&Digest32(source.workbook.sha256).to_hex()), optional(raw), js(&error.reason)));
                    this.evidence.notes.push(format!(
                        "{} (sha256 {}): reader refusal {}; raw {}.",
                        locator,
                        Digest32(source.workbook.sha256),
                        error.reason,
                        raw.unwrap_or("unavailable")
                    ));
                }
                for raw in read.values.into_iter().filter(|v| {
                    v.view == View::Summary
                        || [
                            TargetMetric::CapexTotal,
                            TargetMetric::CapexSustaining,
                            TargetMetric::CapexProjects,
                        ]
                        .iter()
                        .any(|metric| {
                            fmine_public::forecast::record_source_line(v) == metric.source_line()
                        })
                }) {
                    let keyed =
                        key::from_mine_stats(profile, &source.workbook, &source.quarter, &raw)
                            .map_err(|e| e.to_string());
                    let printed = source
                        .workbook
                        .sheets
                        .iter()
                        .find(|s| s.name == raw.sheet)
                        .and_then(|s| s.cells_by_coord.values().find(|c| c.cell_ref == raw.cell))
                        .and_then(|c| c.displayed())
                        .ok_or_else(|| "printed cell is a placeholder or missing".to_string())
                        .and_then(|d| d.map_err(|e| e.reason))
                        .and_then(|d| match parse_token(&d.text())? {
                            Token::Point(n) => decimal(n).map_err(|e| e.to_string()),
                            Token::Range(..) => Err("outcome cell prints a range".into()),
                            Token::AtMost(_) | Token::AtLeast(_) => {
                                Err("outcome cell prints a one-sided bound".into())
                            }
                        });
                    this.observations.push(Observation {
                        release: *quarter,
                        canonical_subject: canonical_raw_subject(profile, &raw),
                        raw,
                        keyed,
                        printed,
                        file: source.file_name.clone(),
                        hash: Digest32(source.workbook.sha256).to_hex(),
                    });
                }
            }
            this.releases.insert(*quarter, release);
        }
        Ok(this)
    }

    fn release_reason(&self, quarter: Quarter, benchmark: bool) -> Option<String> {
        let Some(release) = self.releases.get(&quarter) else {
            return Some("original release was not selected".into());
        };
        release.unreadable.clone().or_else(|| {
            if benchmark {
                release.benchmark_reason.clone()
            } else {
                release.outcome_reason.clone()
            }
        })
    }

    fn lookup(
        &mut self,
        site: &str,
        target: &Target,
        release: Quarter,
        period: Period,
        benchmark: bool,
    ) -> Result<Lookup> {
        let result = self.lookup_inner(site, target, release, period, benchmark)?;
        self.evidence.bindings.push(format!("{{\"role\":{},\"site\":{},\"target\":{},\"release\":{},\"period\":{},\"locators\":[{}],\"unavailable\":{}}}",
            js(if benchmark { "fixed_benchmark_operand" } else { "original_outcome_operand" }), js(site), js(&target.id), js(&release.to_string()), js(&period.code()),
            result.locators.iter().map(|s| js(s)).collect::<Vec<_>>().join(","), optional(reason(&result.value))));
        Ok(result)
    }

    fn lookup_inner(
        &mut self,
        site: &str,
        target: &Target,
        release: Quarter,
        period: Period,
        benchmark: bool,
    ) -> Result<Lookup> {
        let metric = TargetMetric::from_target(target)
            .ok_or_else(|| invalid("source target has no supported interpretation"))?;
        if let Some(reason) = self.release_reason(release, benchmark) {
            return Ok(Lookup::missing(reason));
        }
        let candidates = self
            .observations
            .iter()
            .filter(|o| {
                let field_matches = if metric.is_capex() {
                    o.raw.view == View::Minestats
                        && o.raw.months == 3
                        && fmine_public::forecast::record_source_line(&o.raw)
                            == metric.source_line()
                } else {
                    o.raw.view == View::Summary
                        && (!matches!(period.kind, PeriodKind::Quarter(_)) || o.raw.sheet == "QTD")
                        && o.raw.label == target.field
                };
                let subject = if metric.is_capex() {
                    o.canonical_subject.as_str()
                } else {
                    o.keyed
                        .as_ref()
                        .map_or(o.raw.site.as_str(), |c| c.key.subject.as_str())
                };
                o.release == release
                    && o.raw.period == period.code()
                    && field_matches
                    && subject == site
            })
            .collect::<Vec<_>>();
        let locators = candidates.iter().map(|c| c.locator()).collect::<Vec<_>>();
        for candidate in &candidates {
            self.evidence
                .cells
                .insert(candidate.locator(), candidate.evidence());
            self.evidence
                .markdown
                .insert(candidate.locator(), candidate.markdown());
        }
        let [observation] = candidates.as_slice() else {
            return Ok(Lookup {
                value: unavailable(if candidates.is_empty() {
                    "no original source cell"
                } else {
                    "ambiguous original source cells"
                }),
                basis: None,
                locators,
            });
        };
        let value = match &observation.keyed {
            Err(error) => unavailable(error.clone()),
            Ok(cell) => {
                cell.claim.check_id()?;
                if cell.claim.value_tag() != key::canonical_value_tag(&cell.key, &cell.claim.value)
                {
                    return Err(refusal(
                        RefusalCode::FrozenRecordMutation507,
                        "source interpretation differs from its canonical Claim",
                    ));
                }
                if cell.key.scope != Scope::Site
                    || cell.key.valid_period != period
                    || cell.key.unit != target_unit(target)?
                    || cell.key.line.label != metric.source_line()
                    || cell.key.line.occurrence != 0
                {
                    unavailable(
                        "source unit, support or semantic key differs from the frozen target",
                    )
                } else if cell.key.ownership_basis == OwnershipBasis::Unprinted {
                    unavailable("source ownership basis is unprinted")
                } else if TargetMetric::from_target(target).is_some_and(TargetMetric::is_production)
                    && cell.key.ownership_basis != OwnershipBasis::Consolidated100
                {
                    unavailable("production must use the printed 100% basis")
                } else if metric.is_capex()
                    && !matches!(
                        &cell.claim.value,
                        PublicValue::Number {
                            under_status: None,
                            ..
                        }
                    )
                {
                    unavailable(
                        "capex requires an unflagged printed number, not a status-derived Nil",
                    )
                } else {
                    // A printed numeric zero remains numeric under the frozen rule, even if the generic
                    // public projection calls it a status-dependent Nil. The raw source type remains visible.
                    match (&observation.raw.value, &observation.printed) {
                        (MineStatsCell::Number { .. }, Ok(n)) => {
                            if TargetMetric::from_target(target)
                                .is_some_and(TargetMetric::is_production)
                                && n.signum() < 0
                            {
                                self.evidence.bindings.push(format!("{{\"role\":\"reported_signed_production\",\"locator\":{},\"value\":{},\"interpretation\":\"reported_signed_number_retained_under_frozen_rule;not_physical_endorsement\"}}", js(&observation.locator()), rat(*n)));
                                self.evidence.notes.push(format!("{}: reported signed production {}/{} is retained under the frozen numeric rule; this is not a physical endorsement.", observation.locator(), n.num(), n.den()));
                            }
                            Availability::Available(*n)
                        }
                        (_, Err(error)) => unavailable(error.clone()),
                        _ => unavailable("source cell is not a printed number"),
                    }
                }
            }
        };
        Ok(Lookup {
            value,
            basis: observation
                .keyed
                .as_ref()
                .ok()
                .map(|c| c.key.ownership_basis)
                .filter(|b| *b != OwnershipBasis::Unprinted),
            locators,
        })
    }
}

fn decimal(value: Decimal) -> Result<Rat> {
    let divisor = 10_i128
        .checked_pow(value.scale)
        .ok_or_else(|| invalid("decimal scale overflows"))?;
    exact(Rat::new(value.mantissa, divisor))
}

fn infer_outcome(
    inventory: &mut Inventory,
    site: &str,
    target: &Target,
    quarter: Quarter,
) -> Result<Lookup> {
    let year = u16::try_from(quarter.year)
        .map_err(|_| invalid("outcome year is outside the supported calendar"))?;
    if target.outcome == Outcome::Q4Qtd || target.outcome.is_quarterly() {
        return inventory.lookup(
            site,
            target,
            quarter,
            Period {
                year,
                kind: PeriodKind::Quarter(quarter.q),
            },
            false,
        );
    }
    let annual = inventory.lookup(
        site,
        target,
        quarter,
        Period {
            year,
            kind: PeriodKind::FullYear,
        },
        false,
    )?;
    let nine = inventory.lookup(
        site,
        target,
        Quarter {
            year: i32::from(year),
            q: 3,
        },
        Period {
            year,
            kind: PeriodKind::NineMonths,
        },
        false,
    )?;
    let value = match (number(&annual.value), number(&nine.value)) {
        (Some(fy), Some(ytd)) => {
            let q4 = exact(fy.checked_sub(ytd))?;
            if q4.signum() < 0 {
                inventory.evidence.bindings.push(format!("{{\"role\":\"signed_original_cumulative_residual\",\"site\":{},\"target\":{},\"value\":{},\"interpretation\":\"negative_FY_minus_9M_is_retained_under_the_frozen_rule;not_a_claim_of_negative_physical_production\"}}", js(site), js(&target.id), rat(q4)));
                inventory.evidence.notes.push(format!("{site} {}: original FY minus original 9M is {}/{}. The signed residual remains scored under the frozen target; this is a discrepancy between published cumulative values, not negative physical production.", target.id, q4.num(), q4.den()));
            }
            Availability::Available(q4)
        }
        _ => unavailable(format!(
            "FY minus 9M unavailable: FY {}; 9M {}",
            reason(&annual.value).unwrap_or("available"),
            reason(&nine.value).unwrap_or("available")
        )),
    };
    Ok(Lookup {
        value,
        basis: annual.basis,
        locators: annual.locators.into_iter().chain(nine.locators).collect(),
    })
}

fn persistence(
    inventory: &mut Inventory,
    rules: &RecordRules,
    target: &Target,
    rule: &BaselineRule,
) -> Result<BTreeMap<String, Availability<Bounds>>> {
    let BaselineKind::Persistence {
        pool_from,
        pool_through,
        point,
    } = rule.kind
    else {
        return Err(invalid("persistence resolver needs a persistence rule"));
    };
    let quarters = pool_from.through(pool_through);
    if quarters.len() < 2 || quarters.len() > 200 || pool_through >= point {
        return Err(invalid(
            "persistence pool must contain 2..=200 prior quarters",
        ));
    }
    let capex = TargetMetric::from_target(target).is_some_and(TargetMetric::is_capex);
    let mut capex_points = BTreeMap::new();
    let mut basis_refusal = None;
    let mut series = Vec::new();
    let mut omitted = Vec::new();
    for site in rules.sites.iter().filter(|s| s.status == SiteStatus::Live) {
        let values = quarters
            .iter()
            .map(|q| {
                inventory.lookup(
                    &site.name,
                    target,
                    *q,
                    Period {
                        year: q.year as u16,
                        kind: PeriodKind::Quarter(q.q),
                    },
                    true,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        for (i, pair) in values.windows(2).enumerate() {
            if number(&pair[0].value).is_none_or(|v| v.signum() <= 0)
                || number(&pair[1].value).is_none_or(|v| v.signum() <= 0)
            {
                let pair_reason = |value: &Availability<Rat>| match value {
                    Availability::Unavailable(why) => why.clone(),
                    Availability::Available(value) if value.signum() <= 0 => {
                        "printed value is not positive".into()
                    }
                    Availability::Available(_) => "available_positive".into(),
                };
                omitted.push(format!("{{\"site\":{},\"from\":{},\"through\":{},\"from_reason\":{},\"through_reason\":{}}}", js(&site.name), js(&quarters[i].to_string()), js(&quarters[i + 1].to_string()),
                    js(&pair_reason(&pair[0].value)), js(&pair_reason(&pair[1].value))));
            }
        }
        if capex {
            let point_value = inventory.lookup(
                &site.name,
                target,
                point,
                Period {
                    year: point.year as u16,
                    kind: PeriodKind::Quarter(point.q),
                },
                true,
            )?;
            let bases = values
                .iter()
                .filter(|v| number(&v.value).is_some_and(|n| n.signum() > 0))
                .chain(std::iter::once(&point_value).filter(|v| number(&v.value).is_some()))
                .filter_map(|v| v.basis.map(SeriesOwnershipBasis::from))
                .collect::<BTreeSet<_>>();
            if bases.len() > 1 {
                basis_refusal = Some(format!(
                    "{} {} capex ownership changes across eligible history and point; shared pool refused",
                    site.name, target.id
                ));
            }
            capex_points.insert(site.name.clone(), point_value);
        }
        series.push(values.iter().map(|v| number(&v.value)).collect::<Vec<_>>());
    }
    let (ratios, left_out) = exact(record::pooled_ratios(&series))?;
    let blocked = basis_refusal.or_else(|| {
        quarters.iter().find_map(|q| {
            inventory
                .release_reason(*q, true)
                .map(|why| format!("fixed pool input {q}: {why}"))
        })
    });
    let quantiles = if blocked.is_none() && !ratios.is_empty() {
        Some(exact(record::ratio_quantiles(
            &ratios,
            rules.alpha,
            rules.estimator,
        ))?)
    } else {
        None
    };
    inventory.evidence.pools.push(format!("{{\"target\":{},\"pool_from\":{},\"pool_through\":{},\"point_quarter\":{},\"ratios\":{},\"omitted_pairs\":{},\"quantiles\":{},\"unavailable\":{},\"omissions\":[{}]}}",
        js(&target.id), js(&pool_from.to_string()), js(&pool_through.to_string()), js(&point.to_string()), ratios.len(), left_out,
        quantiles.map_or_else(|| "null".into(), |(lo, hi)| format!("{{\"lo\":{},\"hi\":{}}}", rat(lo), rat(hi))), optional(blocked.as_deref()), omitted.join(",")));
    let mut result = BTreeMap::new();
    for site in rules.sites.iter().filter(|s| s.status == SiteStatus::Live) {
        let point_value = if capex {
            capex_points
                .remove(&site.name)
                .ok_or_else(|| invalid("capex point omitted a frozen member"))?
        } else {
            inventory.lookup(
                &site.name,
                target,
                point,
                Period {
                    year: point.year as u16,
                    kind: PeriodKind::Quarter(point.q),
                },
                true,
            )?
        };
        let value = match (quantiles, number(&point_value.value)) {
            (Some((lo, hi)), Some(point)) => {
                // Only training pairs require positive values. A printed zero is a valid point;
                // signed cost and capex points reverse the order of the scaled quantiles.
                let (lo, hi) = if point.signum() < 0 {
                    (hi, lo)
                } else {
                    (lo, hi)
                };
                Availability::Available(Bounds {
                    lo: exact(point.checked_mul(lo))?,
                    hi: exact(point.checked_mul(hi))?,
                })
            }
            _ => unavailable(blocked.clone().unwrap_or_else(|| {
                reason(&point_value.value)
                    .unwrap_or(if ratios.is_empty() {
                        "no eligible positive consecutive pairs"
                    } else {
                        "persistence point is not a printed number"
                    })
                    .into()
            })),
        };
        result.insert(site.name.clone(), value);
    }
    Ok(result)
}

struct Guidance {
    registry: Option<TranscriptionRegistry>,
    checked: BTreeMap<String, Option<String>>,
}

/// An earlier publication day establishes pre-cut guidance independently of forecast input use.
/// Same-day metadata needs a matching, strictly earlier manifest retrieval, labelled as a declaration.
fn guidance_timing(
    document: &DocumentRow,
    manifest: &RecordManifest,
) -> std::result::Result<&'static str, String> {
    let day = parse_date(&document.published_at).ok_or("guidance publication date is invalid")?;
    let cutoff_day = manifest.information_cutoff_utc.div_euclid(86400);
    if day < cutoff_day {
        return Ok("registered_publication_date_before_cutoff_day");
    }
    if day == cutoff_day
        && manifest_input(manifest, &document.file).is_some_and(|input| {
            parse_hex32(&document.sha256) == Some(input.sha256)
                && input.bytes == document.bytes
                && input.publication_day == day
                && day
                    .checked_mul(86400)
                    .is_some_and(|midnight| input.retrieved_utc >= midnight)
                && input.retrieved_utc < manifest.information_cutoff_utc
        })
    {
        return Ok("manifest_declared_retrieval_before_cutoff");
    }
    Err("guidance is not established strictly before the forecast cutoff; same-day metadata needs earlier manifest-declared retrieval of these exact bytes".into())
}

impl Guidance {
    fn load(root: &Path, profile: &IssuerProfile) -> Result<Self> {
        let path = transcriptions_path(root, &profile.issuer);
        let registry = if path.is_file() {
            Some(
                load_transcriptions(&path, &profile.issuer, &profile.documents)
                    .map_err(|e| invalid(e.to_string()))?,
            )
        } else {
            None
        };
        Ok(Self {
            registry,
            checked: BTreeMap::new(),
        })
    }

    fn check_document(
        &mut self,
        root: &Path,
        document: &DocumentRow,
        manifest: &RecordManifest,
        evidence: &mut Evidence,
    ) -> Result<Option<String>> {
        if let Some(result) = self.checked.get(&document.file) {
            return Ok(result.clone());
        }
        let timing = guidance_timing(document, manifest);
        let mut why = timing.as_ref().err().cloned();
        if why.is_none() {
            let path = root.join(&document.dir).join(&document.file);
            match std::fs::read(&path) {
                Ok(bytes)
                    if bytes.len() as u64 == document.bytes
                        && sha256(&bytes).to_hex() == document.sha256 => {}
                Ok(_) => {
                    return Err(refusal(
                        RefusalCode::DigestMismatch505,
                        format!(
                            "selected guidance document {} differs from its registered bytes",
                            document.file
                        ),
                    ));
                }
                Err(error) => {
                    why = Some(format!(
                        "guidance document {} is unavailable: {error}",
                        document.file
                    ))
                }
            }
        }
        let input = manifest_input(manifest, &document.file);
        evidence.documents.insert(document.file.clone(), format!("{{\"file\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"in_forecast_manifest\":{},\"manifest_declared_retrieval_utc\":{},\"cutoff_timing_basis\":{},\"bytes_verified\":{},\"unavailable\":{}}}",
            js(&document.file), js(&document.sha256), js(&document.published_at), js(&document.published_at_basis), input.is_some(), input.map_or_else(|| "null".into(), |i| js(&format_utc(i.retrieved_utc))), timing.map_or_else(|_| "null".into(), js), why.is_none(), optional(why.as_deref())));
        self.checked.insert(document.file.clone(), why.clone());
        Ok(why)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one frozen baseline selection receives its record, source population, site and target"
    )]
    fn resolve(
        &mut self,
        root: &Path,
        profile: &IssuerProfile,
        manifest: &RecordManifest,
        inventory: &mut Inventory,
        site: &str,
        target: &Target,
        quarter: Quarter,
    ) -> Result<Availability<Bounds>> {
        let Some(registry) = &self.registry else {
            return Ok(unavailable("two-reader guidance registry is absent"));
        };
        let slug = site
            .replace(['í', 'Í'], "i")
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");
        let metric = TargetMetric::from_target(target)
            .ok_or_else(|| invalid("guidance target has no supported source interpretation"))?;
        let quantity = format!("guidance_{slug}_{}", metric.guidance_quantity());
        let year = u16::try_from(quarter.year)
            .map_err(|_| invalid("guidance year is outside the supported calendar"))?;
        let period = if target.outcome.is_quarterly() {
            quarter.to_string()
        } else {
            format!("{year}FY")
        };
        // At midnight that day's guidance cannot strictly precede the cut. Within a day retain the
        // latest candidate even when its timing is uncertain, so it cannot be replaced by older guidance.
        let cut_day = manifest
            .information_cutoff_utc
            .saturating_sub(1)
            .div_euclid(86400);
        let candidates = profile
            .documents
            .iter()
            .filter(|d| {
                parse_date(&d.published_at).is_some_and(|date| date <= cut_day)
                    && registry.entries.iter().any(|e| {
                        e.locator.doc == d.file && e.quantity == quantity && e.period == period
                    })
            })
            .collect::<Vec<_>>();
        let Some(latest) = candidates
            .iter()
            .map(|d| d.published_at.as_str())
            .max()
            .map(str::to_string)
        else {
            return Ok(unavailable(if target.outcome.is_quarterly() {
                "no registered guidance for the scored quarter before the frozen information cut; annual guidance is ineligible"
            } else {
                "no registered annual guidance before the frozen information cut"
            }));
        };
        let documents = candidates
            .into_iter()
            .filter(|d| d.published_at == latest)
            .collect::<Vec<_>>();
        let locators = registry
            .entries
            .iter()
            .filter(|e| {
                documents.iter().any(|d| d.file == e.locator.doc)
                    && e.quantity == quantity
                    && e.period == period
            })
            .map(|e| e.locator.clone())
            .collect::<BTreeSet<_>>();
        let entries = registry
            .entries
            .iter()
            .filter(|e| locators.contains(&e.locator))
            .cloned()
            .collect::<Vec<_>>();
        for entry in &entries {
            let identity = format!("{}:{}", entry.locator, entry.reader);
            inventory
                .evidence
                .cells
                .insert(identity.clone(), transcription_evidence(entry));
            inventory.evidence.markdown.insert(
                identity,
                format!(
                    "| {} | transcription ({}) | `{}` | {} | {} |",
                    crate::guidance::text(&entry.locator.to_string()),
                    crate::guidance::text(&entry.reader),
                    entry.doc_sha256,
                    crate::guidance::text(&entry.token),
                    crate::guidance::text(&format!("{}; {}", entry.unit, entry.basis))
                ),
            );
        }
        let [document] = documents.as_slice() else {
            return Ok(unavailable(
                "multiple guidance documents share the latest pre-cut publication date",
            ));
        };
        if let Some(why) = self.check_document(root, document, manifest, &mut inventory.evidence)? {
            return Ok(unavailable(why));
        }
        let agreed = agree(&TranscriptionRegistry {
            issuer: profile.issuer.clone(),
            entries,
        });
        if !agreed.is_consistent() || !agreed.single_reader.is_empty() {
            return Ok(unavailable(
                "latest pre-cut guidance is disputed or lacks two independent readers",
            ));
        }
        let accepted = agreed
            .accepted
            .iter()
            .filter(|a| a.quantity == quantity && a.period == period)
            .collect::<Vec<_>>();
        let [row] = accepted.as_slice() else {
            return Ok(unavailable(
                "latest pre-cut guidance has no unique accepted quantity",
            ));
        };
        if row.unit != target_unit(target)? {
            return Ok(unavailable("guidance unit differs from the frozen target"));
        }
        let (mut lo, mut hi) = match row.value {
            Token::Point(n) => (decimal(n)?, decimal(n)?),
            Token::Range(lo, hi) => (decimal(lo)?, decimal(hi)?),
            Token::AtMost(_) | Token::AtLeast(_) => {
                return Ok(unavailable(
                    "one-sided guidance cannot supply the frozen finite-interval baseline",
                ));
            }
        };
        let basis_text = if let Some(suffix) = metric.guidance_basis_suffix() {
            let Some(ownership) = row.basis.strip_suffix(suffix) else {
                return Ok(unavailable(if metric.is_capex() {
                    "capex guidance lacks the target's explicit issuer-reported capital scope"
                } else {
                    "cost guidance lacks the target's explicit issuer-reported sold-metal definition"
                }));
            };
            ownership
        } else {
            row.basis.as_str()
        };
        let share = ownership_share(basis_text);
        let Some(share) = share else {
            return Ok(unavailable(
                "guidance ownership basis is unprinted or unsupported",
            ));
        };
        if metric.is_production() {
            lo = exact(lo.checked_div(share))?;
            hi = exact(hi.checked_div(share))?;
        }
        if metric.is_capex() {
            // Quarterly rules bind one common immediately preceding persistence point.
            let point = Quarter {
                year: quarter.year - i32::from(quarter.q == 1),
                q: if quarter.q == 1 { 4 } else { quarter.q - 1 },
            };
            let point_value = inventory.lookup(
                site,
                target,
                point,
                Period {
                    year: point.year as u16,
                    kind: PeriodKind::Quarter(point.q),
                },
                true,
            )?;
            let compatible = match point_value.basis {
                Some(OwnershipBasis::Consolidated100) => basis_text == "100%",
                Some(OwnershipBasis::Attributable { num, den, .. }) => {
                    basis_text.starts_with("attributable:")
                        && share == exact(Rat::new(i128::from(num), i128::from(den)))?
                }
                _ => false,
            };
            if number(&point_value.value).is_none() || !compatible {
                return Ok(unavailable(
                    "capex guidance must match the usable frozen point's printed ownership; no conversion is inferred",
                ));
            }
        }
        if target.outcome == Outcome::FyMinus9m {
            let nine = inventory.lookup(
                site,
                target,
                Quarter {
                    year: i32::from(year),
                    q: 3,
                },
                Period {
                    year,
                    kind: PeriodKind::NineMonths,
                },
                true,
            )?;
            let Some(value) = number(&nine.value) else {
                return Ok(unavailable(format!(
                    "guidance Q4 residual: {}",
                    reason(&nine.value).unwrap_or("9M unavailable")
                )));
            };
            lo = exact(lo.checked_sub(value))?;
            hi = exact(hi.checked_sub(value))?;
        }
        // Annual cost guidance is the pre-registered Q4 baseline unchanged; rates are never subtracted.
        Ok(Availability::Available(Bounds { lo, hi }))
    }
}

fn ownership_share(text: &str) -> Option<Rat> {
    if text == "100%" {
        return Some(Rat::from_integer(1));
    }
    let (n, d) = text.strip_prefix("attributable:")?.split_once('/')?;
    let (n, d) = (n.parse::<i128>().ok()?, d.parse::<i128>().ok()?);
    (n > 0 && d > 0 && n <= d)
        .then(|| Rat::new(n, d).ok())
        .flatten()
}

fn transcription_evidence(entry: &EntryRow) -> String {
    format!(
        "{{\"locator\":{},\"sha256\":{},\"raw_kind\":\"transcription\",\"raw\":{},\"claim_id\":null,\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader\":{}}}",
        js(&entry.locator.to_string()),
        js(&entry.doc_sha256),
        js(&entry.token),
        js(&entry.quantity),
        js(&entry.unit),
        js(&entry.basis),
        js(&entry.period),
        js(&entry.reader)
    )
}

pub(super) fn resolve(
    root: &Path,
    rules: &RecordRules,
    manifest: &RecordManifest,
    as_of_utc: i64,
) -> Result<Resolved> {
    resolve_inner(root, rules, manifest, as_of_utc, None)
}

/// The caller verifies the external proof before supplying its upper time. This adapter binds that
/// bound to the same parsed original-release registry used for outcome selection.
pub(super) fn resolve_nowcast(
    root: &Path,
    rules: &RecordRules,
    manifest: &RecordManifest,
    as_of_utc: i64,
    proof_upper_utc: i64,
) -> Result<Resolved> {
    resolve_inner(root, rules, manifest, as_of_utc, Some(proof_upper_utc))
}

fn resolve_inner(
    root: &Path,
    rules: &RecordRules,
    manifest: &RecordManifest,
    as_of_utc: i64,
    proof_upper_utc: Option<i64>,
) -> Result<Resolved> {
    if as_of_utc < manifest.information_cutoff_utc {
        return Err(refusal(
            RefusalCode::AssayLookahead108,
            "assessment cut precedes frozen information cut",
        ));
    }
    let outcome_quarter = rules
        .outcome_quarter()
        .map_err(|e| invalid(e.to_string()))?;
    let quarterly = rules.targets.iter().any(|t| t.outcome.is_quarterly());
    let post_quarter = rules.is_post_quarter_nowcast();
    if post_quarter {
        if manifest.label != record::Label::Nowcast
            || manifest.information_cutoff_utc < after_quarter(outcome_quarter)?
            || !proof_upper_utc.is_some_and(|upper| {
                upper > manifest.information_cutoff_utc
                    && upper <= rules.dates.proof_by
                    && upper <= as_of_utc
            })
        {
            return Err(refusal(
                RefusalCode::LateRegistration206,
                "post-quarter nowcast needs its explicit label, post-quarter information cut and a verified pre-deadline proof",
            ));
        }
    } else if quarterly && manifest.information_cutoff_utc >= after_quarter(outcome_quarter)? {
        return Err(refusal(
            RefusalCode::LateRegistration206,
            "quarterly record information cut must precede quarter end; post-quarter nowcasts are unsupported",
        ));
    }
    for target in &rules.targets {
        target_unit(target)?;
    }
    if !is_issuer_name(&rules.issuer) {
        return Err(invalid("issuer is not a registry name"));
    }
    let registry = load_sources_registry(&registry_path(root, &rules.issuer))
        .map_err(|e| invalid(e.to_string()))?;
    let nowcast_publications = if post_quarter {
        let original = outcome_quarter.to_string();
        if registry.issuer != rules.issuer
            || registry
                .sources
                .iter()
                .filter(|source| source.kind == "mine_stats" && source.quarter == original)
                .count()
                != 1
        {
            return Err(refusal(
                RefusalCode::SourceCellUnusable208,
                "post-quarter nowcast scoring requires one registered original outcome workbook; no comparative can replace it",
            ));
        }
        let publications =
            fmine_public::forecast::nowcast_publications(&registry, outcome_quarter)?;
        if publications
            .iter()
            .any(|source| proof_upper_utc.is_some_and(|upper| upper >= source.day_start_utc))
        {
            return Err(refusal(
                RefusalCode::LateRegistration206,
                "the complete nowcast proof must precede every registered original publication day",
            ));
        }
        publications
    } else {
        Vec::new()
    };
    let publications = registry
        .sources
        .iter()
        .filter_map(|source| {
            source
                .published_at
                .as_ref()
                .zip(source.published_at_basis.as_ref())
                .map(|(day, basis)| {
                    (
                        source.file.clone(),
                        Publication {
                            day: day.clone(),
                            basis: basis.clone(),
                        },
                    )
                })
        })
        .collect::<BTreeMap<_, _>>();
    let profile = IssuerProfile::from_registry(registry).map_err(invalid)?;
    check_manifest_registry(&profile, manifest, rules)?;
    for input in &manifest.inputs {
        let publication = publications
            .get(&input.doc_id)
            .map(|p| p.day.as_str())
            .or_else(|| {
                profile
                    .documents
                    .iter()
                    .find(|d| d.file == input.doc_id)
                    .map(|d| d.published_at.as_str())
            });
        if let Some(day) = publication {
            let published =
                parse_date(day).ok_or_else(|| invalid("registered publication date is invalid"))?;
            if let Some(source) = profile.sources.iter().find(|s| s.file_name == input.doc_id) {
                let quarter = Quarter::parse(&source.quarter)
                    .ok_or_else(|| invalid("registered workbook quarter is invalid"))?;
                if published < after_quarter(quarter)?.div_euclid(86400) {
                    return Err(refusal(
                        RefusalCode::AssayLookahead108,
                        format!(
                            "{} has a registered publication before its declared quarter finished",
                            input.doc_id
                        ),
                    ));
                }
            }
            if published > manifest.information_cutoff_utc.div_euclid(86400)
                || published > input.retrieved_utc.div_euclid(86400)
            {
                return Err(refusal(
                    RefusalCode::AssayLookahead108,
                    format!(
                        "{} has a registered publication after the frozen retrieval or information cut",
                        input.doc_id
                    ),
                ));
            }
        }
    }
    let mut outcome_quarters = BTreeSet::from([outcome_quarter]);
    // Annual records retain their original two-release inventory, including rate-only records.
    if !quarterly {
        outcome_quarters.insert(Quarter {
            year: outcome_quarter.year,
            q: 3,
        });
    }
    let mut wanted = outcome_quarters.clone();
    for baseline in &rules.baselines {
        if let BaselineKind::Persistence {
            pool_from,
            pool_through,
            point,
        } = baseline.kind
        {
            if pool_from > pool_through
                || pool_through >= point
                || pool_through.year - pool_from.year > 50
            {
                return Err(invalid(
                    "persistence pool is not a bounded historical interval",
                ));
            }
            wanted.extend(pool_from.through(pool_through));
            wanted.insert(point);
        }
    }
    let mut inventory = Inventory::load(
        root,
        &profile,
        manifest,
        &wanted,
        &publications,
        &outcome_quarters,
        as_of_utc,
    )?;
    for source in &nowcast_publications {
        inventory.evidence.bindings.push(format!(
            "{{\"role\":\"nowcast_publication_boundary\",\"release\":{},\"file\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"publication_day_start_utc\":{},\"verified_record_proof_upper_utc\":{}}}",
            js(&outcome_quarter.to_string()),
            js(&source.file),
            js(&source.sha256),
            js(&source.published_at),
            js(&source.published_at_basis),
            js(&format_utc(source.day_start_utc)),
            proof_upper_utc.map_or_else(|| "null".into(), |upper| js(&format_utc(upper))),
        ));
    }
    if let Some(availability) = &rules.nowcast_availability {
        inventory.evidence.bindings.push(format!(
            "{{\"role\":\"declared_earliest_outcome_release\",\"earliest_release_utc\":{},\"basis\":{},\"proof_deadline_utc\":{},\"independently_verified\":false}}",
            js(&format_utc(availability.earliest_release_utc)), js(&availability.basis),
            js(&format_utc(rules.dates.proof_by)),
        ));
        inventory.evidence.notes.push(format!(
            "Declared earliest outcome release (including preliminary announcements): {}. Basis: {}. Frozen proof deadline: {}. This declaration is not proof that earlier publications do not exist.",
            format_utc(availability.earliest_release_utc), availability.basis, format_utc(rules.dates.proof_by),
        ));
    }
    if post_quarter {
        inventory.evidence.notes.push("Post-quarter nowcast, scored separately from advance forecasts. The verified record proof precedes every registered original workbook/HTML publication day; registration completeness and publication citations are not independently authenticated.".into());
    }
    let mut guidance = Guidance::load(root, &profile)?;
    let mut outcomes = Vec::new();
    let mut baselines = Vec::new();
    for target in &rules.targets {
        let capex = TargetMetric::from_target(target).is_some_and(TargetMetric::is_capex);
        for site in &rules.sites {
            let mut observed = if site.status == SiteStatus::NotPublished {
                Lookup::missing(format!(
                    "frozen NOT_PUBLISHED: {}",
                    site.reason
                        .as_deref()
                        .unwrap_or("record universe exclusion")
                ))
            } else {
                infer_outcome(&mut inventory, &site.name, target, outcome_quarter)?
            };
            if TargetMetric::from_target(target).is_some_and(|m| !m.is_production())
                && site.status == SiteStatus::Live
            {
                let reference = rules.baselines.iter().find_map(|b| match b.kind {
                    BaselineKind::Persistence {
                        pool_through,
                        point,
                        ..
                    } if b.target == target.id => Some(if capex { point } else { pool_through }),
                    _ => None,
                });
                let old = if let Some(q) = reference {
                    let previous = inventory.lookup(
                        &site.name,
                        target,
                        q,
                        Period {
                            year: q.year as u16,
                            kind: PeriodKind::Quarter(q.q),
                        },
                        true,
                    )?;
                    if capex && number(&previous.value).is_none() {
                        None
                    } else {
                        previous.basis
                    }
                } else {
                    None
                };
                let changed = observed.basis.zip(old).map(|(now, before)| {
                    SeriesOwnershipBasis::from(now) != SeriesOwnershipBasis::from(before)
                });
                inventory.evidence.bindings.push(format!("{{\"role\":\"outcome_basis_comparison\",\"site\":{},\"target\":{},\"basis_changed\":{},\"reference_basis\":{},\"outcome_basis\":{}}}",
                    js(&site.name), js(&target.id), changed.map_or_else(|| "null".into(), |v| v.to_string()), old.map_or_else(|| "null".into(), |v| js(&basis(v))), observed.basis.map_or_else(|| "null".into(), |v| js(&basis(v)))));
                if capex {
                    if number(&observed.value).is_some() && changed != Some(false) {
                        observed.value = unavailable(
                            "capex outcome needs the same printed ownership as the usable frozen point; no conversion is inferred",
                        );
                    }
                    inventory.evidence.notes.push(format!(
                        "{} {} capex retains its printed scope, quarterly support and ownership. Outcome ownership must match the usable frozen point; status-derived Nil is not zero.",
                        site.name, target.id
                    ));
                } else {
                    inventory.evidence.notes.push(format!("{} {} ownership basis: frozen pool-through {}; outcome {}; basis_changed={}. Cost rates retain their printed basis and are not rescaled.",
                        site.name, target.id, old.map_or_else(|| "unavailable".into(), basis), observed.basis.map_or_else(|| "unavailable".into(), basis), changed.map_or_else(|| "unknown".into(), |v| v.to_string())));
                }
            }
            outcomes.push(OutcomeRow {
                site: site.name.clone(),
                target: target.id.clone(),
                value: observed.value,
            });
        }
        for baseline in rules.baselines.iter().filter(|b| b.target == target.id) {
            let mut persisted = if matches!(baseline.kind, BaselineKind::Persistence { .. }) {
                persistence(&mut inventory, rules, target, baseline)?
            } else {
                BTreeMap::new()
            };
            for site in &rules.sites {
                let value = if site.status == SiteStatus::NotPublished {
                    unavailable(format!(
                        "frozen NOT_PUBLISHED: {}",
                        site.reason
                            .as_deref()
                            .unwrap_or("record universe exclusion")
                    ))
                } else {
                    match baseline.kind {
                        BaselineKind::Persistence { .. } => persisted
                            .remove(&site.name)
                            .ok_or_else(|| invalid("persistence result omitted a frozen member"))?,
                        BaselineKind::IssuerGuidance => guidance.resolve(
                            root,
                            &profile,
                            manifest,
                            &mut inventory,
                            &site.name,
                            target,
                            outcome_quarter,
                        )?,
                    }
                };
                inventory.evidence.bindings.push(format!("{{\"role\":\"baseline_resolution\",\"site\":{},\"target\":{},\"baseline\":{},\"unavailable\":{}}}", js(&site.name), js(&target.id), js(&baseline.id), optional(reason(&value))));
                baselines.push(BaselineRow {
                    site: site.name.clone(),
                    target: target.id.clone(),
                    baseline: baseline.id.clone(),
                    value,
                });
            }
        }
    }
    let evidence = inventory.evidence;
    let has_capex = rules
        .targets
        .iter()
        .any(|target| TargetMetric::from_target(target).is_some_and(TargetMetric::is_capex));
    let point_name = if has_capex {
        "original quarterly persistence-point"
    } else if quarterly {
        "original QTD persistence-point"
    } else {
        "Q3"
    };
    let evidence_markdown = format!(
        "## Source bindings\n\nOriginal releases supply outcomes and the fixed benchmark observations by the assessment cut. Guidance itself must be strictly before the forecast information cutoff; same-day guidance requires an earlier retrieval declared in the exact frozen manifest. Fixed {point_name} and historical benchmark observations may follow that cutoff and need not be forecast inputs. The methods are not asserted to have the same information set. New source availability uses an explicit registered publication date when available (conservatively through the end of that day), otherwise the registry retrieval bound. Retrieval is never relabelled as publication. Printed zero remains numeric under the frozen record rule.\n\n| Source locator | Claim or reader | SHA-256 | Raw token | Unit, basis and interpretation |\n|---|---|---|---|---|\n{}\n\n{}\n",
        evidence
            .markdown
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
        evidence
            .notes
            .iter()
            .map(|s| crate::guidance::text(s))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    let outcome_policy = if has_capex {
        "original_scored_quarter_QTD_for_production_and_unit_cost;original_scored_quarter_Minestats_for_capex;never_cumulative_or_later_comparatives"
    } else if quarterly {
        "original_scored_quarter_QTD_for_production_and_unit_cost;never_cumulative_or_later_comparatives"
    } else {
        "original_yearQ4_FY_minus_original_yearQ3_9M_for_production;original_yearQ4_QTD_for_unit_cost;never_later_comparatives"
    };
    let baseline_policy = if quarterly {
        "matching_quarter_guidance_strictly_before_forecast_cut;annual_guidance_ineligible;fixed_original_point_and_history_by_assessment_cut_may_be_outside_forecast_manifest"
    } else {
        "guidance_strictly_before_forecast_cut;fixed_original_Q3_and_history_by_assessment_cut_may_be_outside_forecast_manifest"
    };
    let printed_zero_policy = if has_capex {
        "legacy_six_targets_keep_frozen_numeric_zero;capex_requires_unflagged_numeric_source"
    } else {
        "numeric_zero_is_an_outcome_or_persistence_point_even_when_generic_projection_has_status_nil"
    };
    let evidence_json = format!(
        "{{\"outcome_policy\":{},\"baseline_policy\":{},\"same_information_set_claim\":false,\"verification_scope\":\"all matching manifest hashes checked against registry; only selected source bytes re-read\",\"printed_zero_policy\":{},\"sources\":[{}],\"cells\":[{}],\"documents\":[{}],\"bindings\":[{}],\"persistence_pools\":[{}]}}",
        js(outcome_policy),
        js(baseline_policy),
        js(printed_zero_policy),
        evidence.sources.join(","),
        evidence.cells.into_values().collect::<Vec<_>>().join(","),
        evidence
            .documents
            .into_values()
            .collect::<Vec<_>>()
            .join(","),
        evidence.bindings.join(","),
        evidence.pools.join(",")
    );
    Ok(Resolved {
        outcomes,
        baselines,
        evidence_json,
        evidence_markdown,
        synthetic: profile.issuer == "synthetic",
        outcome_quarter,
    })
}

#[cfg(test)]
#[path = "../../../xtask/testdata/forecast_record_sources.rs"]
mod tests;
