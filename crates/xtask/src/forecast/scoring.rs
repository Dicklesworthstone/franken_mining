//! Source-bound outcomes for externally timestamped public forecast records.
//! Determinism: Exact (frozen bytes, canonical source Claims and rational arithmetic).
//!
//! Q4 production is the original FY total less the original nine-month total. Q4 AISC is the original
//! three-month rate; subtracting annual and nine-month rates is forbidden. Later comparatives are never
//! substituted. A missing release or cell remains an explicit unresolved member of the frozen universe.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_calib::record::{
    BaselineKind, Outcome, Rat, RecordRules, SiteStatus, Target,
    scoring::{self, OutcomeRow, TargetScore},
};
use fmine_public::LoadedSource;
use fmine_public::key::{
    KeyedCell, NilReason, OwnershipBasis, Period, PeriodKind, PublicValue, Scope,
    SeriesOwnershipBasis, canonical_value_tag,
};
use fmine_public::sources::{IssuerProfile, SourceKind};

use super::{Res, exact_text, fail, load, parse_utc, verify_loaded};

fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn rational(x: Option<Rat>) -> String {
    x.map_or_else(|| "null".into(), |v| js(&exact_text(v)))
}

fn names(values: &[String]) -> String {
    format!(
        "[{}]",
        values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
    )
}

fn year(rules: &RecordRules) -> Res<u16> {
    let mut years = BTreeSet::new();
    for baseline in &rules.baselines {
        if let BaselineKind::Persistence { point, .. } = baseline.kind {
            if point.q != 3 {
                return fail("outcome scoring requires Q3 persistence point quarters".into());
            }
            years.insert(point.year);
        }
    }
    let [year] = years.into_iter().collect::<Vec<_>>()[..] else {
        return fail(
            "outcome scoring requires one common Q3 point year in the frozen baseline rules".into(),
        );
    };
    u16::try_from(year).map_err(|_| vec!["outcome year is outside the supported calendar".into()])
}

/// This first adapter implements the two frozen FY2026 target meanings. Unsupported fields refuse;
/// arbitrary rates cannot be made additive by changing an outcome selector in a new rules file.
fn expected_unit(target: &Target) -> Res<&'static str> {
    match (&target.outcome, target.field.as_str()) {
        (Outcome::FyMinus9m, "au_prod_100_koz") => Ok("koz"),
        (Outcome::Q4Qtd, "aisc_usd_per_oz") => Ok("usd_per_oz"),
        _ => fail(format!(
            "unsupported frozen outcome rule for {} / {}",
            target.id, target.field
        )),
    }
}

fn original<'a>(
    profile: &IssuerProfile,
    cells: &'a [KeyedCell],
    year: u16,
    quarter: u8,
    kind: PeriodKind,
    site: &str,
    target: &Target,
) -> Res<Option<&'a KeyedCell>> {
    let release = format!("{year}Q{quarter}");
    let sources: Vec<_> = profile
        .sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats && s.quarter == release)
        .collect();
    if sources.len() > 1 {
        return fail(format!(
            "{release}: ambiguous original-release registration"
        ));
    }
    let Some(source) = sources.first() else {
        return Ok(None);
    };
    let label = format!("summary:{}", target.field);
    let mut hits = cells.iter().filter(|c| {
        c.key.issuer == profile.issuer
            && c.key.subject == site
            && c.key.valid_period == Period { year, kind }
            && c.key.reporting_vintage.release == release
            && c.key.reporting_vintage.file_sha256 == source.sha256_hex
            && c.key.line.label == label
    });
    let first = hits.next();
    if hits.next().is_some() {
        return fail(format!(
            "{release} {site} {}: ambiguous source cells",
            target.field
        ));
    }
    if let Some(cell) = first {
        cell.claim.check_id().map_err(|e| vec![e.to_string()])?;
        if cell.claim.metric_key != cell.key.line.label
            || cell.claim.value_tag() != canonical_value_tag(&cell.key, &cell.claim.value)
            || !cell.claim.confidentiality.is_public()
            || cell.claim.stale
            || cell.claim.calibration_failed
            || cell.key.scope != Scope::Site
            || cell.key.unit != expected_unit(target)?
            || !cell.claim.anchors.iter().any(|a| {
                fmine_core::hash::Digest32(a.artifact_sha256).to_hex() == source.sha256_hex
                    && Path::new(&a.artifact_path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        == Some(source.file_name.as_str())
            })
        {
            return fail(format!(
                "{}: source Claim, scope, unit or anchor does not match the frozen target",
                cell.locator
            ));
        }
        if target.outcome == Outcome::FyMinus9m
            && cell.key.ownership_basis != OwnershipBasis::Consolidated100
        {
            return fail(format!(
                "{}: production must be explicitly on the 100% basis",
                cell.locator
            ));
        }
    }
    Ok(first)
}

/// A record-specific projection of the same verified workbook bytes that produced the canonical Claims.
/// Status classification and stored precision remain in those Claims. The frozen rule scores displayed numbers, including
/// numeric zero, while a zero formatted as a dash is still a published placeholder.
#[derive(Debug, Clone)]
struct ReportedNumber {
    stored: String,
    displayed: Result<Option<Rat>, String>,
}

type ReportedNumbers = BTreeMap<(String, String), ReportedNumber>;

fn reported_numbers(sources: &[LoadedSource]) -> ReportedNumbers {
    let mut out = BTreeMap::new();
    for source in sources {
        let digest = fmine_core::hash::Digest32(source.workbook.sha256).to_hex();
        for sheet in &source.workbook.sheets {
            for cell in sheet.cells_by_coord.values() {
                let Some(stored) = &cell.number_text else {
                    continue;
                };
                let displayed = match cell.displayed() {
                    Some(Ok(display)) => {
                        let text = display.text();
                        match Rat::from_decimal_str(&text) {
                            Ok(value) => Ok(Some(value)),
                            Err(_)
                                if matches!(text.trim(), "" | "-" | "–" | "—" | "n/a" | "N/A") =>
                            {
                                Ok(None)
                            }
                            Err(_) => {
                                Err("display is neither a number nor a registered placeholder"
                                    .into())
                            }
                        }
                    }
                    Some(Err(error)) => {
                        Err(format!("display interpretation refused: {}", error.code))
                    }
                    None => Err("stored number has no numeric source-cell interpretation".into()),
                };
                out.insert(
                    (
                        digest.clone(),
                        format!("{}:{}!{}", source.file_name, sheet.name, cell.cell_ref),
                    ),
                    ReportedNumber {
                        stored: stored.clone(),
                        displayed,
                    },
                );
            }
        }
    }
    out
}

fn numeric(cell: &KeyedCell, reported: &ReportedNumbers) -> Res<Option<Rat>> {
    let stored = match &cell.claim.value {
        PublicValue::Number { stored, .. } => stored,
        PublicValue::Nil {
            token,
            reason: NilReason::Disposed | NilReason::Withheld,
        } => {
            // The shared classifier uses these variants for a numeric zero under a subject status.
            // The independent source-cell projection below must corroborate both its bytes and display.
            if !Rat::from_decimal_str(token).is_ok_and(|value| value.signum() == 0) {
                return fail(format!(
                    "{}: status-marked token is not a source numeric zero",
                    cell.locator
                ));
            }
            token
        }
        _ => return Ok(None),
    };
    let key = (
        cell.key.reporting_vintage.file_sha256.clone(),
        cell.locator.clone(),
    );
    let source = reported.get(&key).ok_or_else(|| {
        vec![format!(
            "{}: no matching verified reported-number projection",
            cell.locator
        )]
    })?;
    if source.stored != *stored {
        return fail(format!(
            "{}: reported number differs from its canonical source token",
            cell.locator
        ));
    }
    source
        .displayed
        .clone()
        .map_err(|e| vec![format!("{}: {e}", cell.locator)])
}

struct BoundOutcome<'a> {
    site: String,
    target: String,
    value: Option<Rat>,
    status: &'static str,
    reason: String,
    sources: Vec<(&'static str, &'a KeyedCell)>,
    basis_changed: Option<bool>,
}

fn resolve<'a>(
    profile: &IssuerProfile,
    cells: &'a [KeyedCell],
    year: u16,
    site: &str,
    target: &Target,
    basis_reference: Option<&'a KeyedCell>,
    reported: &ReportedNumbers,
) -> Res<BoundOutcome<'a>> {
    expected_unit(target)?;
    let mut row = BoundOutcome {
        site: site.into(),
        target: target.id.clone(),
        value: None,
        status: "not_published",
        reason: "the original release has no usable numeric outcome for this frozen member".into(),
        sources: Vec::new(),
        basis_changed: None,
    };
    let needed: &[u8] = match target.outcome {
        Outcome::FyMinus9m => &[3, 4],
        Outcome::Q4Qtd => &[4],
    };
    if needed.iter().any(|q| {
        !profile
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::MineStats && s.quarter == format!("{year}Q{q}"))
    }) {
        row.status = "release_not_registered";
        row.reason =
            "an original outcome release is not registered; later comparatives are ineligible"
                .into();
    }
    match target.outcome {
        Outcome::FyMinus9m => {
            let fy = original(profile, cells, year, 4, PeriodKind::FullYear, site, target)?;
            let nine = original(
                profile,
                cells,
                year,
                3,
                PeriodKind::NineMonths,
                site,
                target,
            )?;
            if let Some(cell) = fy {
                row.sources.push(("full_year_original", cell));
            }
            if let Some(cell) = nine {
                row.sources.push(("nine_months_original", cell));
            }
            if let (Some(fy), Some(nine)) = (fy, nine)
                && let (Some(fy), Some(nine)) = (numeric(fy, reported)?, numeric(nine, reported)?)
            {
                // The frozen target is this signed difference. A negative result cannot introduce
                // an unregistered exclusion after the original cumulative releases are observed.
                row.value = Some(fy.checked_sub(nine).map_err(|e| vec![e.to_string()])?);
            }
        }
        Outcome::Q4Qtd => {
            if let Some(cell) = original(
                profile,
                cells,
                year,
                4,
                PeriodKind::Quarter(4),
                site,
                target,
            )? {
                row.sources.push(("q4_rate_original", cell));
                row.value = numeric(cell, reported)?;
                if let Some(reference) = basis_reference {
                    row.sources.push(("frozen_q2_basis_reference", reference));
                    if cell.printed_basis != OwnershipBasis::Unprinted
                        && reference.printed_basis != OwnershipBasis::Unprinted
                    {
                        row.basis_changed = Some(
                            SeriesOwnershipBasis::from(cell.printed_basis)
                                != SeriesOwnershipBasis::from(reference.printed_basis),
                        );
                    }
                }
            }
        }
    }
    if row.value.is_some() {
        row.status = "resolved";
        row.reason.clear();
    }
    Ok(row)
}

fn summary_json(s: &TargetScore) -> String {
    let completion = s.coverage_completion.map_or_else(
        || "null".into(),
        |(lo, hi)| format!("[{},{}]", rational(Some(lo)), rational(Some(hi))),
    );
    format!(
        "{{\"target\":{},\"n_universe\":{},\"n_frozen\":{},\"n_scored\":{},\"n_unresolved\":{},\
         \"not_forecast\":{},\"unresolved\":{},\"covered\":{},\"observed_coverage\":{},\
         \"full_cohort_coverage_bounds\":{},\"iid_coverage_reference\":[{},{}],\
         \"observed_mean_interval_score\":{},\"full_mean_interval_score_lower\":{},\
         \"full_mean_interval_score_upper\":{},\"effective_n\":[{},{}],\"uncalibrated\":true}}",
        js(&s.target),
        s.n_universe,
        s.n_frozen,
        s.n_scored,
        s.n_unresolved,
        names(&s.not_forecast),
        names(&s.unresolved),
        s.covered,
        rational(s.observed_coverage),
        completion,
        rational(Some(s.iid_coverage_reference.lo)),
        rational(Some(s.iid_coverage_reference.hi)),
        rational(s.observed_mean_interval_score),
        rational(s.full_mean_interval_score_lower),
        rational(s.full_mean_interval_score_upper),
        rational(Some(s.effective_n.0)),
        rational(Some(s.effective_n.1)),
    )
}

/// Assess a verified record against byte-verified registered original releases. Source bodies are loaded only
/// after the record and prior rules timestamp have authenticated the complete frozen forecast universe.
/// This command never creates forecasts, registrations, timestamps, outcomes or baseline values.
pub fn record_score(root: &Path, dir: &Path) -> Res<Vec<String>> {
    let loaded = load(root, dir)?;
    let (verification, proof) = verify_loaded(root, dir, &loaded)?;
    let year = year(&loaded.rules)?;
    let end = parse_utc(&format!("{}-01-01T00:00:00Z", u32::from(year) + 1))
        .ok_or_else(|| vec!["unsupported outcome calendar year".into()])?;
    if proof.upper_utc >= end {
        return fail("record proof must precede the end of the forecast year; an old outcome cannot be registered retrospectively".into());
    }
    for target in &loaded.rules.targets {
        expected_unit(target)?;
    }
    let mut profile = fmine_public::load_issuer_profile(root, &loaded.rules.issuer)
        .map_err(|e| vec![e.to_string()])?;
    let reference_is_frozen = profile.sources.iter().any(|s| {
        s.kind == SourceKind::MineStats
            && s.quarter == format!("{year}Q2")
            && loaded.manifest.inputs.iter().any(|i| {
                i.doc_id == s.file_name
                    && i.bytes == s.bytes
                    && fmine_core::hash::Digest32(i.sha256).to_hex() == s.sha256_hex
                    && i.url == s.url
            })
    });
    // Only original outcomes and a manifest-bound Q2 ownership reference can affect this report.
    // Later comparative workbooks are not even opened, and cannot fill a missing original.
    profile.sources.retain(|s| {
        s.kind == SourceKind::MineStats
            && (s.quarter == format!("{year}Q3")
                || s.quarter == format!("{year}Q4")
                || (reference_is_frozen && s.quarter == format!("{year}Q2")))
    });
    let sources =
        fmine_public::load_registered_sources(&profile, root).map_err(|e| vec![e.to_string()])?;
    let vocabulary: BTreeSet<_> = profile.mine_stats_labels.iter().cloned().collect();
    let mut reads = Vec::new();
    for source in &sources {
        let read = fmine_public::mine_stats::read_mine_stats(
            &source.workbook,
            &source.quarter,
            (!vocabulary.is_empty()).then_some(&vocabulary),
        )
        .map_err(|e| vec![e.to_string()])?;
        reads.push((source.file_name.clone(), read));
    }
    let keyed = fmine_public::key::mine_stats::build_claims(&profile, &sources, &reads);
    let reported = reported_numbers(&sources);
    if !keyed.refusals.is_empty() {
        return Err(keyed
            .refusals
            .iter()
            .map(|r| format!("{}: source read refused: {}", r.locator, r.reason))
            .collect());
    }
    let mut bound = Vec::new();
    let mut outcomes = Vec::new();
    for target in &loaded.rules.targets {
        for site in &loaded.rules.sites {
            let row = if site.status == SiteStatus::NotPublished {
                BoundOutcome {
                    site: site.name.clone(),
                    target: target.id.clone(),
                    value: None,
                    status: "not_forecast",
                    reason: site
                        .reason
                        .clone()
                        .unwrap_or_else(|| "not published in the frozen universe".into()),
                    sources: Vec::new(),
                    basis_changed: None,
                }
            } else {
                let reference = if target.outcome == Outcome::Q4Qtd && reference_is_frozen {
                    original(
                        &profile,
                        &keyed.claims,
                        year,
                        2,
                        PeriodKind::Quarter(2),
                        &site.name,
                        target,
                    )?
                } else {
                    None
                };
                let row = resolve(
                    &profile,
                    &keyed.claims,
                    year,
                    &site.name,
                    target,
                    reference,
                    &reported,
                )?;
                outcomes.push(OutcomeRow {
                    site: row.site.clone(),
                    target: row.target.clone(),
                    value: row.value,
                });
                row
            };
            bound.push(row);
        }
    }
    let scores = scoring::assess(&loaded.rules, &loaded.forecasts, &outcomes)
        .map_err(|e| vec![e.to_string()])?;
    let mut rows = Vec::new();
    for (row, score) in bound.iter().zip(&scores.members) {
        let forecast = loaded
            .forecasts
            .iter()
            .find(|f| f.site == row.site && f.target == row.target);
        let forecast = forecast.map_or_else(
            || "null".into(),
            |f| {
                format!(
                    "{{\"point\":{},\"lower\":{},\"upper\":{}}}",
                    rational(Some(f.point)),
                    rational(Some(f.lo)),
                    rational(Some(f.hi))
                )
            },
        );
        let evidence = row
            .sources
            .iter()
            .map(|(role, cell)| format!("{{\"role\":{},\"source\":{}}}", js(role), cell.to_json()))
            .collect::<Vec<_>>()
            .join(",");
        rows.push(format!(
            "{{\"site\":{},\"target\":{},\"status\":{},\"reason\":{},\"forecast\":{},\
             \"outcome\":{},\"interval_score\":{},\"covered\":{},\"basis_changed\":{},\"sources\":[{}]}}",
            js(&row.site), js(&row.target), js(row.status), js(&row.reason), forecast,
            rational(row.value), rational(score.interval_score),
            score.covered.map_or_else(|| "null".into(), |x| x.to_string()),
            row.basis_changed.map_or_else(|| "null".into(), |x| x.to_string()), evidence,
        ));
    }
    let baselines = loaded
        .rules
        .baselines
        .iter()
        .map(|b| {
            format!(
                "{{\"id\":{},\"target\":{},\"status\":\"not_assessed\"}}",
                js(&b.id),
                js(&b.target)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "{{\"schema\":\"fmine.forecast-record-assessment.v1\",\"record_id\":{},\"issuer\":{},\
         \"label\":{},\"record_sha256\":{},\"rules_sha256\":{},\"source_claims_digest\":{},\
         \"proof_lower_utc\":{},\"proof_upper_utc\":{},\"verification\":{},\
         \"outcome_vintage\":\"first_published\",\"numeric_basis\":\"source_workbook_displayed_decimal\",\
         \"alpha\":{},\"coverage_level\":{},\"rho_range\":[{},{}],\"uncalibrated\":true,\
         \"calibration_claim\":false,\"rows\":[{}],\"targets\":[{}],\"registered_baselines\":[{}],\
         \"registered_three_way_comparison\":\"not_assessed\"}}",
        js(&loaded.manifest.record_id),
        js(&loaded.rules.issuer),
        js(loaded.manifest.label.as_str()),
        js(&loaded.digest),
        js(&loaded.rules_digest),
        js(&keyed.digest.to_hex()),
        js(&super::format_utc(proof.lower_utc)),
        js(&super::format_utc(proof.upper_utc)),
        names(&verification),
        rational(Some(loaded.rules.alpha)),
        rational(Some(loaded.rules.coverage_level)),
        rational(Some(loaded.rules.rho_low)),
        rational(Some(loaded.rules.rho_high)),
        rows.join(","),
        scores
            .targets
            .iter()
            .map(summary_json)
            .collect::<Vec<_>>()
            .join(","),
        baselines,
    );
    let digest = fmine_core::hash::sha256(body.as_bytes()).to_hex();
    Ok(vec![format!(
        "{{\"assessment_sha256\":{},\"report\":{body}}}",
        js(&digest)
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::Digest32;
    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx,
        build_minimal_xlsx_styled,
    };
    use fmine_public::key::mine_stats::build_claims;
    use fmine_public::key::{StatusKind, SubjectStatus};
    use fmine_public::mine_stats::read_mine_stats;
    use fmine_public::sources::PrintedShareCorrection;
    use fmine_public::sources::RegisteredSource;

    fn add_workbook(
        profile: &mut IssuerProfile,
        reported: &mut ReportedNumbers,
        file: &str,
        release: &str,
        bytes: &[u8],
    ) -> Vec<KeyedCell> {
        let workbook = Workbook::from_bytes(file, bytes).unwrap();
        let read = read_mine_stats(&workbook, release, None).unwrap();
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        profile.sources.push(RegisteredSource {
            file_name: file.into(),
            kind: SourceKind::MineStats,
            quarter: release.into(),
            url: format!("https://example.invalid/{file}"),
            url_basis: "synthetic test".into(),
            bytes: bytes.len() as u64,
            sha256_hex: Digest32(workbook.sha256).to_hex(),
        });
        let source = LoadedSource {
            file_name: file.into(),
            kind: SourceKind::MineStats,
            quarter: release.into(),
            url: format!("https://example.invalid/{file}"),
            url_basis: "synthetic test".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        };
        reported.extend(reported_numbers(std::slice::from_ref(&source)));
        let keyed = build_claims(profile, &[source], &[(file.into(), read)]);
        assert!(keyed.refusals.is_empty(), "{:?}", keyed.refusals);
        keyed.claims
    }

    fn fixture(
        profile: &mut IssuerProfile,
        reported: &mut ReportedNumbers,
        release: &str,
        year: u16,
        full: f64,
        qtd_cost: f64,
    ) -> Vec<KeyedCell> {
        let text = |s: &str| MinimalCell::Text(s.into());
        let q = release.as_bytes()[5] - b'0';
        let ytd = match q {
            2 => "For the six months ended June 30,",
            3 => "For the nine months ended September 30,",
            _ => "For the twelve months ended December 31,",
        };
        let qtd = match q {
            2 => "For the three months ended June 30,",
            3 => "For the three months ended September 30,",
            _ => "For the three months ended December 31,",
        };
        let sheet = |name: &str, label: &str, production: f64, cost: f64| MinimalSheet {
            name: name.into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text(label)),
                ("B1".into(), MinimalCell::Number(f64::from(year))),
                ("B2".into(), text("Gold produced (000s oz) (100%)")),
                ("C2".into(), text("All-in sustaining costs ($/oz)")),
                (
                    "A3".into(),
                    text(if q == 2 {
                        "Example Mine (80%)"
                    } else {
                        "Example Mine (60%)"
                    }),
                ),
                ("B3".into(), MinimalCell::Number(production)),
                ("C3".into(), MinimalCell::Number(cost)),
            ],
        };
        let bytes = build_minimal_xlsx(&[
            sheet("YTD", ytd, full, 1400.0),
            sheet("QTD", qtd, 999.0, qtd_cost),
        ]);
        add_workbook(
            profile,
            reported,
            &format!("invented_{release}.xlsx"),
            release,
            &bytes,
        )
    }

    struct Data {
        profile: IssuerProfile,
        cells: Vec<KeyedCell>,
        rules: RecordRules,
        reported: ReportedNumbers,
    }

    fn empty_data() -> Data {
        let mut profile = fmine_public::synthetic::synthetic_profile();
        profile.sources.clear();
        profile.subject_statuses.clear();
        let rules = fmine_calib::record::parse_rules(include_str!(
            "../../../../forecasts/fy2026/rules.toml"
        ))
        .unwrap();
        Data {
            profile,
            cells: Vec::new(),
            rules,
            reported: BTreeMap::new(),
        }
    }

    fn data() -> Data {
        let mut d = empty_data();
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2030Q3",
            2030,
            300.0,
            1300.0,
        ));
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2030Q4",
            2030,
            400.0,
            1600.0,
        ));
        // A later comparative prints a different prior-year total and rate.
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2031Q4",
            2030,
            900.0,
            2500.0,
        ));
        d
    }

    fn row<'a>(d: &'a Data, site: &str, target: &Target) -> Res<BoundOutcome<'a>> {
        resolve(&d.profile, &d.cells, 2030, site, target, None, &d.reported)
    }

    #[test]
    fn original_workbook_claims_resolve_additive_production_and_direct_q4_rate() {
        let d = data();
        let production = row(&d, "Example Mine", &d.rules.targets[0]).unwrap();
        assert_eq!(production.value, Some(Rat::from_integer(100)));
        assert_eq!(production.sources.len(), 2);
        assert!(
            production.sources.iter().all(|(_, c)| c
                .key
                .reporting_vintage
                .release
                .starts_with("2030"))
        );
        let cost = row(&d, "Example Mine", &d.rules.targets[1]).unwrap();
        assert_eq!(cost.value, Some(Rat::from_integer(1600)));
        assert_eq!(cost.sources.len(), 1);
        assert_eq!(
            cost.sources[0].1.key.valid_period.kind,
            PeriodKind::Quarter(4)
        );
        for (_, source) in production.sources.iter().chain(&cost.sources) {
            source.claim.check_id().unwrap();
            assert!(!source.claim.anchors.is_empty());
        }
    }

    #[test]
    fn missing_original_release_never_selects_later_comparatives() {
        let mut d = data();
        d.profile.sources.retain(|s| s.quarter != "2030Q4");
        for target in &d.rules.targets {
            let actual = row(&d, "Example Mine", target).unwrap();
            assert_eq!(actual.value, None);
            assert_eq!(actual.status, "release_not_registered");
        }
    }

    #[test]
    fn source_mutations_and_rate_subtraction_are_refused() {
        let d = data();
        for mode in 0..3 {
            let mut changed = d.cells.clone();
            let c = changed
                .iter_mut()
                .find(|c| {
                    c.key.reporting_vintage.release == "2030Q4"
                        && c.key.valid_period.kind == PeriodKind::FullYear
                        && c.key.line.label == "summary:au_prod_100_koz"
                })
                .unwrap();
            match mode {
                0 => c.key.unit = "oz".into(),
                1 => c.key.ownership_basis = OwnershipBasis::Unprinted,
                _ => {
                    if let PublicValue::Number { stored, .. } = &mut c.claim.value {
                        *stored = "99999".into();
                    }
                }
            }
            assert!(
                resolve(
                    &d.profile,
                    &changed,
                    2030,
                    "Example Mine",
                    &d.rules.targets[0],
                    None,
                    &d.reported
                )
                .is_err()
            );
        }
        let mut rate = d.rules.targets[1].clone();
        rate.outcome = Outcome::FyMinus9m;
        assert!(row(&d, "Example Mine", &rate).is_err());
    }

    #[test]
    fn missing_cells_remain_explicit_and_zero_is_a_real_outcome() {
        let d = data();
        let absent = row(&d, "Absent Mine", &d.rules.targets[0]).unwrap();
        assert_eq!(absent.value, None);
        assert_eq!(absent.status, "not_published");
        let mut zero = empty_data();
        zero.cells.extend(fixture(
            &mut zero.profile,
            &mut zero.reported,
            "2030Q3",
            2030,
            300.0,
            0.0,
        ));
        zero.cells.extend(fixture(
            &mut zero.profile,
            &mut zero.reported,
            "2030Q4",
            2030,
            300.0,
            0.0,
        ));
        for target in &zero.rules.targets {
            assert_eq!(
                row(&zero, "Example Mine", target).unwrap().value,
                Some(Rat::from_integer(0))
            );
        }
    }

    #[test]
    fn signed_frozen_difference_is_scored_without_an_unregistered_exclusion() {
        let mut d = empty_data();
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2030Q3",
            2030,
            300.0,
            1300.0,
        ));
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2030Q4",
            2030,
            200.0,
            1600.0,
        ));
        let actual = row(&d, "Example Mine", &d.rules.targets[0]).unwrap();
        assert_eq!(actual.status, "resolved");
        assert_eq!(actual.value, Some(Rat::from_integer(-100)));
        let mut rules = d.rules.clone();
        rules.sites.truncate(1);
        rules.sites[0].name = "Example Mine".into();
        rules.targets.truncate(1);
        rules.baselines.retain(|b| b.target == "production");
        let forecast = fmine_calib::record::ForecastRow {
            site: "Example Mine".into(),
            target: "production".into(),
            point: Rat::from_integer(100),
            lo: Rat::from_integer(90),
            hi: Rat::from_integer(110),
        };
        let outcome = OutcomeRow {
            site: "Example Mine".into(),
            target: "production".into(),
            value: actual.value,
        };
        let scored = scoring::assess(&rules, &[forecast], &[outcome]).unwrap();
        assert_eq!(scored.targets[0].n_scored, 1);
        // Width 20 plus 10 times the 190-unit lower-tail miss.
        assert_eq!(
            scored.members[0].interval_score,
            Some(Rat::from_integer(1920))
        );
    }

    #[test]
    fn printed_share_changes_remain_visible_when_effective_shares_match() {
        let mut d = empty_data();
        d.profile
            .printed_share_corrections
            .push(PrintedShareCorrection {
                subject: "Example Mine".into(),
                printed: "80".into(),
                num: 3,
                den: 5,
                evidence: "synthetic correction of the old printed label".into(),
            });
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2030Q2",
            2030,
            200.0,
            1400.0,
        ));
        d.cells.extend(fixture(
            &mut d.profile,
            &mut d.reported,
            "2030Q4",
            2030,
            400.0,
            1600.0,
        ));
        let target = &d.rules.targets[1];
        let reference = original(
            &d.profile,
            &d.cells,
            2030,
            2,
            PeriodKind::Quarter(2),
            "Example Mine",
            target,
        )
        .unwrap()
        .unwrap();
        let actual = resolve(
            &d.profile,
            &d.cells,
            2030,
            "Example Mine",
            target,
            Some(reference),
            &d.reported,
        )
        .unwrap();
        let current = actual.sources[0].1;
        assert_eq!(
            SeriesOwnershipBasis::from(current.key.ownership_basis),
            SeriesOwnershipBasis::from(reference.key.ownership_basis),
        );
        assert_ne!(current.printed_basis, reference.printed_basis);
        assert_eq!(actual.basis_changed, Some(true));
    }

    #[test]
    fn status_marked_numbers_and_zeros_follow_the_frozen_rule_but_dashes_do_not() {
        let mut d = empty_data();
        for (site, status) in [
            ("Disposed Zero", StatusKind::Disposed),
            ("Withheld Zero", StatusKind::Withheld),
            ("Flagged Number", StatusKind::Disposed),
            ("Formatted Dash", StatusKind::Disposed),
        ] {
            d.profile.subject_statuses.push(SubjectStatus::from_date(
                site,
                status,
                (2029, 12, 1),
                "synthetic status",
            ));
        }
        let text = |s: &str| MinimalCell::Text(s.into());
        let sheets = [MinimalSheet {
            name: "QTD".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("For the three months ended December 31,")),
                ("B1".into(), MinimalCell::Number(2030.0)),
                ("B2".into(), text("All-in sustaining costs ($/oz)")),
                ("A3".into(), text("Disposed Zero (60%)")),
                ("B3".into(), MinimalCell::Number(0.0)),
                ("A4".into(), text("Withheld Zero (60%)")),
                ("B4".into(), MinimalCell::Number(0.0)),
                ("A5".into(), text("Flagged Number (60%)")),
                ("B5".into(), MinimalCell::Number(25.0)),
                ("A6".into(), text("Dash Token (60%)")),
                ("B6".into(), text("—")),
                ("A7".into(), text("Formatted Dash (60%)")),
                (
                    "B7".into(),
                    MinimalCell::Styled {
                        raw: "0".into(),
                        xf: 1,
                    },
                ),
                ("A8".into(), text("Not Applicable (60%)")),
                ("B8".into(), text("n/a")),
                ("A9".into(), text("Missing (60%)")),
                ("A10".into(), text("Unsupported Display (60%)")),
                (
                    "B10".into(),
                    MinimalCell::Styled {
                        raw: "1".into(),
                        xf: 2,
                    },
                ),
            ],
        }];
        let styles = MinimalStyles {
            num_fmts: vec![(164, "0;-0;\"—\"".into()), (165, "0.0E+00".into())],
            xfs: vec![0, 164, 165],
        };
        let bytes = build_minimal_xlsx_styled(&sheets, Some(&styles));
        d.cells = add_workbook(
            &mut d.profile,
            &mut d.reported,
            "status_values.xlsx",
            "2030Q4",
            &bytes,
        );
        let before = d.cells.clone();
        let target = &d.rules.targets[1];
        for (site, expected) in [
            ("Disposed Zero", 0),
            ("Withheld Zero", 0),
            ("Flagged Number", 25),
        ] {
            let actual = row(&d, site, target).unwrap();
            assert_eq!(actual.value, Some(Rat::from_integer(expected)));
            assert_eq!(actual.status, "resolved");
        }
        let zero = row(&d, "Disposed Zero", target).unwrap();
        assert!(matches!(
            zero.sources[0].1.claim.value,
            PublicValue::Nil {
                reason: NilReason::Disposed,
                ..
            }
        ));
        let number = row(&d, "Flagged Number", target).unwrap();
        assert!(matches!(
            number.sources[0].1.claim.value,
            PublicValue::Number {
                under_status: Some(StatusKind::Disposed),
                ..
            }
        ));
        for site in ["Dash Token", "Formatted Dash", "Not Applicable", "Missing"] {
            let actual = row(&d, site, target).unwrap();
            assert_eq!(actual.value, None, "{site}");
            assert_eq!(actual.status, "not_published");
        }
        assert!(row(&d, "Unsupported Display", target).is_err());
        assert_eq!(
            d.cells, before,
            "record projection must not rewrite the shared source interpretation"
        );
        d.reported.clear();
        assert!(
            row(&d, "Disposed Zero", &d.rules.targets[1]).is_err(),
            "a status-zero Claim alone cannot replace its source display evidence"
        );
    }
    #[test]
    fn scored_outcome_uses_the_displayed_decimal_and_retains_stored_precision() {
        let mut d = empty_data();
        let text = |s: &str| MinimalCell::Text(s.into());
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "QTD".into(),
                hidden: false,
                cells: vec![
                    ("A1".into(), text("For the three months ended December 31,")),
                    ("B1".into(), MinimalCell::Number(2030.0)),
                    ("B2".into(), text("All-in sustaining costs ($/oz)")),
                    ("A3".into(), text("Example Mine (60%)")),
                    (
                        "B3".into(),
                        MinimalCell::Styled {
                            raw: "1600.49".into(),
                            xf: 1,
                        },
                    ),
                ],
            }],
            Some(&MinimalStyles {
                num_fmts: vec![(164, "0.0".into())],
                xfs: vec![0, 164],
            }),
        );
        d.cells = add_workbook(
            &mut d.profile,
            &mut d.reported,
            "rounded_values.xlsx",
            "2030Q4",
            &bytes,
        );
        let actual = row(&d, "Example Mine", &d.rules.targets[1]).unwrap();
        assert_eq!(actual.value, Some(Rat::new(3201, 2).unwrap()));
        assert!(matches!(
            &actual.sources[0].1.claim.value,
            PublicValue::Number { stored, displayed: Ok(shown), .. }
                if stored == "1600.49" && shown == "1600.5"
        ));
        assert_ne!(actual.value, Some(Rat::new(160049, 100).unwrap()));
    }
}
