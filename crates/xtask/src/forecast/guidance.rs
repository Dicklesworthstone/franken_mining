//! DESIGN (BD-334, BD-067): source-bound issuer-guidance comparators under the frozen forecast rules.
//! Determinism: Exact.
//! A comparator is not a new forecast or an external timestamp. Missing members remain in the population.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use fmine_calib::record::{self, BaselineKind, Outcome, Rat, RecordRules, SiteStatus, Target};
use fmine_core::hash::{Digest32, sha256};
use fmine_public::key::{self, KeyedCell, OwnershipBasis, PeriodKind, Scope};
use fmine_public::sources::{IssuerProfile, SourceKind};
use fmine_public::transcribe::{CheckedDocument, TranscriptionReport, load_transcription_prefixes};
use fmine_registry::sources::is_date;
use fmine_registry::transcriptions::{Accepted, Decimal, Token, parse_token};

use super::{FY2026_RULES, Res, exact_text, fail, parse_rules_bytes, read};

pub const NO_CLAIM: &str = "A comparator computed under the frozen rules from registered public evidence; not a FrankenMining forecast, calibration claim, historical registration or external timestamp proof. Workbook retrieval is an availability upper bound, not a publication date.";

#[derive(Debug, Clone)]
pub struct GuidanceEvidence {
    pub reading: Accepted,
    pub document: CheckedDocument,
}

#[derive(Debug, Clone)]
pub struct BaselineRow {
    pub site: String,
    pub target: String,
    pub status: &'static str,
    pub reason: String,
    pub unit: String,
    pub basis: String,
    /// Exact printed endpoints under the pre-registered transformation; negative remainders are not clamped.
    pub interval: Option<(Rat, Rat)>,
    pub guidance: Vec<GuidanceEvidence>,
    pub pending_readings: Vec<String>,
    pub actuals: Vec<KeyedCell>,
}

#[derive(Debug, Clone)]
pub struct BaselineReport {
    pub issuer: String,
    pub rules_id: String,
    pub rules_digest: Digest32,
    pub as_of: String,
    pub workbooks_available_by: String,
    pub rows: Vec<BaselineRow>,
}

fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl GuidanceEvidence {
    fn json(&self) -> String {
        let a = &self.reading;
        format!(
            "{{\"at\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"quantity\":{},\"period\":{},\"unit\":{},\"basis\":{},\"token\":{},\"readers\":[{}]}}",
            js(&a.locator.to_string()),
            js(&a.doc_sha256),
            js(&self.document.published_at),
            js(&self.document.published_at_basis),
            js(&a.quantity),
            js(&a.period),
            js(&a.unit),
            js(&a.basis),
            js(&a.token),
            a.readers
                .iter()
                .map(|r| js(r))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

impl BaselineRow {
    pub(super) fn json(&self) -> String {
        let interval = self.interval.map_or_else(
            || "null".into(),
            |(lo, hi)| {
                format!(
                    "{{\"lower\":{},\"upper\":{}}}",
                    js(&exact_text(lo)),
                    js(&exact_text(hi))
                )
            },
        );
        format!(
            "{{\"baseline\":\"issuer_guidance\",\"site\":{},\"target\":{},\"status\":{},\"reason\":{},\"unit\":{},\"basis\":{},\"interval\":{},\"guidance\":[{}],\"pending_readings\":[{}],\"actuals\":[{}]}}",
            js(&self.site),
            js(&self.target),
            js(self.status),
            js(&self.reason),
            js(&self.unit),
            js(&self.basis),
            interval,
            self.guidance
                .iter()
                .map(GuidanceEvidence::json)
                .collect::<Vec<_>>()
                .join(","),
            self.pending_readings
                .iter()
                .map(|s| js(s))
                .collect::<Vec<_>>()
                .join(","),
            self.actuals
                .iter()
                .map(KeyedCell::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

impl BaselineReport {
    /// A header binds the exact source-bearing rows. Deterministic JSON lines retain every declared member.
    pub fn json_lines(&self) -> Vec<String> {
        let rows: Vec<String> = self.rows.iter().map(BaselineRow::json).collect();
        let body = format!(
            "FMBL1\n{}\n{}\n{}\n{}\n",
            self.rules_digest.to_hex(),
            self.as_of,
            self.workbooks_available_by,
            rows.join("\n")
        );
        let mut lines = vec![format!(
            "{{\"schema\":\"fmine.forecast-baselines.v1\",\"issuer\":{},\"synthetic\":{},\"rules_id\":{},\"rules_sha256\":{},\"as_of\":{},\"cut_semantics\":\"inclusive_publication_date\",\"workbooks_available_by\":{},\"n_members\":{},\"n_available\":{},\"digest\":{},\"no_claim\":{}}}",
            js(&self.issuer),
            self.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER,
            js(&self.rules_id),
            js(&self.rules_digest.to_hex()),
            js(&self.as_of),
            js(&self.workbooks_available_by),
            self.rows.len(),
            self.rows.iter().filter(|r| r.interval.is_some()).count(),
            js(&sha256(body.as_bytes()).to_hex()),
            js(NO_CLAIM)
        )];
        lines.extend(rows);
        lines
    }
}

fn availability(profile: &IssuerProfile, as_of: &str) -> Res<String> {
    let date = profile
        .retrieved_utc
        .split('/')
        .filter_map(|s| s.get(..10))
        .filter(|s| is_date(s))
        .max()
        .ok_or_else(|| vec!["workbooks have no dated availability evidence".into()])?;
    if date > as_of {
        return fail(format!(
            "this baseline requires verified retrieval {date}, which cannot support evidence cut {as_of}"
        ));
    }
    // Every registered workbook belongs to a completed quarter; a future quarter cannot enter persistence.
    for source in &profile.sources {
        let q = record::Quarter::parse(&source.quarter)
            .ok_or_else(|| vec!["invalid workbook quarter".into()])?;
        let end = format!(
            "{:04}-{:02}-{}",
            q.year,
            q.q * 3,
            if matches!(q.q, 1 | 4) { "31" } else { "30" }
        );
        if end.as_str() > date {
            return fail(format!(
                "{}: quarter end {end} follows its registered availability bound {date}",
                source.file_name
            ));
        }
    }
    Ok(date.to_string())
}

fn key(site: &str) -> String {
    site.replace(['í', 'Í'], "i")
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn specification(target: &Target) -> Res<(&'static str, &'static str)> {
    match (target.field.as_str(), target.outcome) {
        ("au_prod_100_koz", Outcome::FyMinus9m) => Ok(("production_koz", "koz")),
        ("aisc_usd_per_oz", Outcome::Q4Qtd) => Ok(("aisc_usd_per_oz", "usd_per_oz")),
        _ => fail(format!(
            "{}: issuer-guidance baseline has no declared adapter for {} {:?}",
            target.id, target.field, target.outcome
        )),
    }
}

fn decimal(d: Decimal) -> Result<Rat, String> {
    let den = 10_i128
        .checked_pow(d.scale)
        .ok_or("decimal scale overflow")?;
    Rat::new(d.mantissa, den).map_err(|e| e.to_string())
}

fn endpoints(a: &Accepted, production: bool) -> Result<(Rat, Rat, String), String> {
    if parse_token(&a.token)? != a.value {
        return Err("guidance token differs from the agreed interpretation".into());
    }
    let ownership = if production {
        a.basis.as_str()
    } else {
        a.basis
            .strip_suffix(";aisc=issuer-reported-per-sold-oz")
            .ok_or("AISC guidance must declare issuer-reported cost per sold ounce")?
    };
    let share = if ownership == "100%" {
        Rat::from_integer(1)
    } else {
        let (n, d) = ownership
            .strip_prefix("attributable:")
            .and_then(|s| s.split_once('/'))
            .ok_or("guidance ownership must be 100% or attributable:n/d")?;
        let (n, d) = (
            n.parse::<i128>().map_err(|_| "invalid share numerator")?,
            d.parse::<i128>().map_err(|_| "invalid share denominator")?,
        );
        if n <= 0 || d <= 0 || n > d {
            return Err("ownership share outside (0,1]".into());
        }
        Rat::new(n, d).map_err(|e| e.to_string())?
    };
    let (lo, hi) = match a.value {
        Token::Point(d) => (decimal(d)?, decimal(d)?),
        Token::Range(lo, hi) => (decimal(lo)?, decimal(hi)?),
    };
    if production {
        Ok((
            lo.checked_div(share).map_err(|e| e.to_string())?,
            hi.checked_div(share).map_err(|e| e.to_string())?,
            "100%".into(),
        ))
    } else {
        Ok((lo, hi, a.basis.clone()))
    }
}

fn select(
    row: &mut BaselineRow,
    report: Option<&TranscriptionReport>,
    quantity: &str,
    period: &str,
) -> Result<(), String> {
    let Some(report) = report else {
        return Err("no registered two-reader guidance".into());
    };
    let mut candidates = Vec::new();
    for a in report
        .agreement
        .accepted
        .iter()
        .filter(|a| a.quantity == quantity && a.period == period)
    {
        let d = report
            .documents
            .iter()
            .find(|d| d.file == a.locator.doc)
            .ok_or("guidance has no verified document")?;
        if a.doc_sha256 != d.sha256 || a.readers.iter().collect::<BTreeSet<_>>().len() < 2 {
            return Err("guidance lacks two independent readings of the verified bytes".into());
        }
        candidates.push(GuidanceEvidence {
            reading: a.clone(),
            document: d.clone(),
        });
    }
    let latest = candidates
        .iter()
        .map(|c| c.document.published_at.clone())
        .max();
    row.guidance = candidates
        .into_iter()
        .filter(|c| Some(&c.document.published_at) == latest.as_ref())
        .collect();
    row.guidance
        .sort_by(|a, b| a.reading.locator.cmp(&b.reading.locator));
    for (at, _) in &report.agreement.single_reader {
        if report
            .agreement
            .rejected_quantities
            .get(at)
            .is_some_and(|qs| qs.contains(quantity))
            && report.documents.iter().any(|d| {
                d.file == at.doc && latest.as_ref().is_none_or(|date| &d.published_at >= date)
            })
        {
            row.pending_readings.push(at.to_string());
        }
    }
    row.pending_readings.sort();
    if !row.pending_readings.is_empty() {
        return Err(
            "latest guidance has pending single-reader evidence; older guidance is not substituted"
                .into(),
        );
    }
    match row.guidance.len() {
        0 => Err("no agreed guidance for this target and year before the cut".into()),
        1 => Ok(()),
        _ => Err("multiple guidance values share the latest publication date".into()),
    }
}

fn check_actual(cell: &KeyedCell) -> Result<(), String> {
    cell.claim.check_id().map_err(|e| e.to_string())?;
    if cell.claim.metric_key != cell.key.line.label
        || cell.claim.value_tag() != key::canonical_value_tag(&cell.key, &cell.claim.value)
        || cell.key.ownership_basis != OwnershipBasis::Consolidated100
        || cell.key.scope != Scope::Site
        || cell.key.unit != "koz"
        || !cell.claim.confidentiality.is_public()
        || cell.claim.stale
        || cell.claim.calibration_failed
    {
        return Err(
            "nine-month Claim has mismatched identity, unit, scope, basis or public evidence"
                .into(),
        );
    }
    Ok(())
}

/// The frozen guidance transformation over already verified source evidence. The caller establishes the
/// information cutoff and supplies the original-release displayed-number projection. This does not read a
/// broader registry or infer a probability level for the issuer's range.
pub(super) fn bind_rows(
    rules: &RecordRules,
    year: u16,
    transcripts: Option<&TranscriptionReport>,
    cells: &[KeyedCell],
    mut number: impl FnMut(&KeyedCell) -> Result<Rat, String>,
    mut eligible: impl FnMut(&GuidanceEvidence) -> Result<(), String>,
) -> Res<Vec<BaselineRow>> {
    let period = format!("{year}FY");
    let original_release = format!("{year}Q3");
    let mut rows = Vec::new();
    for baseline in rules
        .baselines
        .iter()
        .filter(|b| b.kind == BaselineKind::IssuerGuidance)
    {
        let target = rules
            .targets
            .iter()
            .find(|t| t.id == baseline.target)
            .ok_or_else(|| vec!["baseline target is undeclared".into()])?;
        let (metric, unit) = specification(target)?;
        for site in &rules.sites {
            let mut row = BaselineRow {
                site: site.name.clone(),
                target: target.id.clone(),
                status: "not_available",
                reason: String::new(),
                unit: unit.into(),
                basis: String::new(),
                interval: None,
                guidance: Vec::new(),
                pending_readings: Vec::new(),
                actuals: Vec::new(),
            };
            if site.status == SiteStatus::NotPublished {
                row.status = "not_published";
                row.reason = site
                    .reason
                    .clone()
                    .unwrap_or_else(|| "excluded by frozen site universe".into());
                rows.push(row);
                continue;
            }
            let quantity = format!("guidance_{}_{metric}", key(&site.name));
            let resolved = (|| {
                select(&mut row, transcripts, &quantity, &period)?;
                eligible(&row.guidance[0])?;
                let a = &row.guidance[0].reading;
                if a.unit != unit {
                    return Err(format!("guidance unit {} differs from {unit}", a.unit));
                }
                let production = target.outcome == Outcome::FyMinus9m;
                let (mut lo, mut hi, basis) = endpoints(a, production)?;
                row.basis = basis;
                if production {
                    row.actuals = cells
                        .iter()
                        .filter(|c| {
                            c.key.subject == site.name
                                && c.key.valid_period.year == year
                                && c.key.valid_period.kind == PeriodKind::NineMonths
                                && c.key.reporting_vintage.release == original_release
                                && c.key.line.label == "summary:au_prod_100_koz"
                        })
                        .cloned()
                        .collect();
                    let [cell] = row.actuals.as_slice() else {
                        return Err(if row.actuals.is_empty() {
                            "original Q3-release nine-month production is not available"
                        } else {
                            "original nine-month production is ambiguous"
                        }
                        .into());
                    };
                    check_actual(cell)?;
                    let nine_months = number(cell)?;
                    lo = lo.checked_sub(nine_months).map_err(|e| e.to_string())?;
                    hi = hi.checked_sub(nine_months).map_err(|e| e.to_string())?;
                }
                row.interval = Some((lo, hi));
                Ok::<(), String>(())
            })();
            match resolved {
                Ok(()) => {
                    row.status = "available";
                    row.reason = baseline.rule.clone();
                }
                Err(reason) => row.reason = reason,
            }
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Evaluates the fixed issuer-guidance rule over checked files; no forecast or guidance value is synthesized.
pub fn run(root: &Path, as_of: &str) -> Res<BaselineReport> {
    if !is_date(as_of) {
        return fail("forecast-baseline --as-of needs YYYY-MM-DD".into());
    }
    let bytes = read(&root.join(FY2026_RULES))?;
    let rules = parse_rules_bytes(&bytes, FY2026_RULES)?;
    let profile =
        fmine_public::load_issuer_profile(root, &rules.issuer).map_err(|e| vec![e.to_string()])?;
    let available = availability(&profile, as_of)?;
    let year = record::format_utc(rules.dates.nowcast_from)
        .get(..4)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| vec!["rules have no four-digit target year".into()])?;
    let original_release = format!("{year}Q3");
    let transcripts = load_transcription_prefixes(&profile, root, &["guidance_"], Some(as_of))
        .map_err(|e| vec![e.to_string()])?;
    let mut original = profile.clone();
    original
        .sources
        .retain(|s| s.kind == SourceKind::MineStats && s.quarter == original_release);
    let (claims, reported) = super::scoring::observations(&original, root)?;
    let rows = bind_rows(
        &rules,
        year,
        transcripts.as_ref(),
        &claims.claims,
        |cell| {
            super::scoring::numeric(cell, &reported)
                .map_err(|errors| errors.join("; "))?
                .ok_or_else(|| "original nine-month cell is not displayed as a number".into())
        },
        |_| Ok(()),
    )?;
    Ok(BaselineReport {
        issuer: rules.issuer,
        rules_id: rules.rules_id,
        rules_digest: record::rules_digest(&bytes),
        as_of: as_of.into(),
        workbooks_available_by: available,
        rows,
    })
}
