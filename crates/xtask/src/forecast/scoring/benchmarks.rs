//! Registered comparator information sets for the authenticated forecast-record assessment.
//! Determinism: Exact. Comparator evidence is optional: a refusal preserves the model's outcome scores.
//! Fixed Q3 observations and historical persistence pools need not be forecast-manifest inputs. Guidance,
//! however, must precede the frozen cutoff. These different information sets are disclosed, not equated.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use fmine_calib::record::{
    self, BaselineKind, BaselineRule, Quarter, Rat, RecordManifest, SiteStatus,
    scoring::{self, BenchmarkRow, ComparisonExclusion, OutcomeRow, TargetComparison},
};
use fmine_core::hash::Digest32;
use fmine_public::key::mine_stats::KeyedClaims;
use fmine_public::key::{KeyedCell, PeriodKind};
use fmine_public::sources::{IssuerProfile, SourceKind};
use fmine_public::transcribe::{CheckedDocument, load_transcription_prefixes};

use super::super::{Loaded, Res, format_utc, guidance, parse_utc};
use super::{ReportedNumbers, js, names, numeric, observations, original, rational};

fn interval_json(interval: Option<(Rat, Rat)>) -> String {
    interval.map_or_else(
        || "null".into(),
        |(lo, hi)| {
            format!(
                "{{\"lower\":{},\"upper\":{}}}",
                rational(Some(lo)),
                rational(Some(hi))
            )
        },
    )
}

/// Date-only publication metadata cannot order a release within the cutoff day. An exact document
/// bound in the frozen manifest can instead establish the author's declared retrieval before the cut;
/// that declaration is reported as such, never as an independent timestamp of the document.
fn guidance_timing(
    document: &CheckedDocument,
    manifest: &RecordManifest,
) -> Result<&'static str, String> {
    let midnight = parse_utc(&format!("{}T00:00:00Z", document.published_at))
        .ok_or("guidance has no valid publication date")?;
    let day = midnight.div_euclid(86_400);
    let cutoff_day = manifest.information_cutoff_utc.div_euclid(86_400);
    if day < cutoff_day {
        return Ok("registered_publication_date_before_cutoff_day");
    }
    if day == cutoff_day
        && manifest.inputs.iter().any(|input| {
            input.doc_id == document.file
                && Digest32(input.sha256).to_hex() == document.sha256
                && input.publication_day == day
                && input.retrieved_utc >= midnight
                && input.retrieved_utc < manifest.information_cutoff_utc
        })
    {
        return Ok("manifest_declared_retrieval_before_cutoff");
    }
    Err("guidance publication is not established strictly before the information cutoff; same-day metadata lacks an earlier manifest-declared retrieval of these exact bytes".into())
}

/// At midnight no document published on that day can be strictly before the cutoff. At any later
/// instant, retain that day's candidates so an uncertain latest reading cannot be replaced by an older one.
fn guidance_date_bound(cutoff: i64) -> String {
    format_utc(cutoff.saturating_sub(1))[..10].to_string()
}

fn input_for<'a>(cell: &KeyedCell, manifest: &'a RecordManifest) -> Option<&'a record::InputDoc> {
    manifest.inputs.iter().find(|input| {
        Digest32(input.sha256).to_hex() == cell.key.reporting_vintage.file_sha256
            && cell.claim.anchors.iter().any(|anchor| {
                anchor.artifact_sha256 == input.sha256
                    && Path::new(&anchor.artifact_path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        == Some(input.doc_id.as_str())
            })
    })
}

fn observation_json(cell: &KeyedCell, manifest: &RecordManifest) -> String {
    let input = input_for(cell, manifest);
    format!(
        "{{\"in_forecast_manifest\":{},\"manifest_declared_retrieval_utc\":{},\"source\":{}}}",
        input.is_some(),
        input.map_or_else(
            || "null".into(),
            |input| js(&format_utc(input.retrieved_utc))
        ),
        cell.to_json(),
    )
}

fn guidance_json(row: &guidance::BaselineRow, manifest: &RecordManifest) -> String {
    let documents = row.guidance.iter().map(|evidence| {
        let declared = manifest.inputs.iter().find(|input| {
            input.doc_id == evidence.document.file
                && Digest32(input.sha256).to_hex() == evidence.document.sha256
        });
        let (status, reason) = match guidance_timing(&evidence.document, manifest) {
            Ok(basis) => (basis, None),
            Err(reason) => ("not_established", Some(reason)),
        };
        format!(
            "{{\"file\":{},\"sha256\":{},\"in_forecast_manifest\":{},\"cutoff_timing_basis\":{},\"reason\":{},\"manifest_declared_retrieval_utc\":{}}}",
            js(&evidence.document.file), js(&evidence.document.sha256), declared.is_some(), js(status),
            reason.as_deref().map_or_else(|| "null".into(), js),
            declared.map_or_else(|| "null".into(), |input| js(&format_utc(input.retrieved_utc))),
        )
    }).collect::<Vec<_>>().join(",");
    let actuals = row
        .actuals
        .iter()
        .map(|cell| observation_json(cell, manifest))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"row\":{},\"nominal_probability\":null,\"document_timing\":[{}],\"fixed_observations\":[{}]}}",
        row.json(),
        documents,
        actuals,
    )
}

fn missing_guidance(loaded: &Loaded, reason: &str) -> Vec<guidance::BaselineRow> {
    loaded
        .rules
        .baselines
        .iter()
        .filter(|baseline| baseline.kind == BaselineKind::IssuerGuidance)
        .flat_map(|baseline| {
            loaded.rules.sites.iter().map(move |site| {
                let excluded = site.status == SiteStatus::NotPublished;
                guidance::BaselineRow {
                    site: site.name.clone(),
                    target: baseline.target.clone(),
                    status: if excluded { "not_published" } else { "refused" },
                    reason: if excluded {
                        site.reason
                            .clone()
                            .unwrap_or_else(|| "excluded by frozen site universe".into())
                    } else {
                        reason.into()
                    },
                    unit: String::new(),
                    basis: String::new(),
                    interval: None,
                    guidance: Vec::new(),
                    pending_readings: Vec::new(),
                    actuals: Vec::new(),
                }
            })
        })
        .collect()
}

type History = (KeyedClaims, ReportedNumbers);

/// Missing registrations are not a license to shorten the fixed training pool. A registered unreadable
/// workbook refuses the optional comparator, while the separately authenticated outcomes remain usable.
fn history(root: &Path, profile: &IssuerProfile, from: Quarter, through: Quarter) -> Res<History> {
    let quarters: Vec<String> = from
        .through(through)
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut selected = profile.clone();
    selected.sources.retain(|source| {
        source.kind == SourceKind::MineStats && quarters.contains(&source.quarter)
    });
    let problems: Vec<String> = quarters.iter().filter_map(|quarter| {
        let n = selected.sources.iter().filter(|source| &source.quarter == quarter).count();
        (n != 1).then(|| format!("{quarter}: fixed persistence pool requires one original registration; found {n}"))
    }).collect();
    if !problems.is_empty() {
        return Err(problems);
    }
    observations(&selected, root)
}

struct PersistenceMember {
    site: String,
    status: &'static str,
    reason: String,
    point: Option<Rat>,
    interval: Option<(Rat, Rat)>,
    source: Option<KeyedCell>,
}

struct Persistence {
    target: String,
    from: Quarter,
    through: Quarter,
    point: Quarter,
    status: &'static str,
    reason: String,
    ratios: Vec<Rat>,
    excluded_pairs: Option<usize>,
    quantiles: Option<(Rat, Rat)>,
    pool_observations: Vec<String>,
    members: Vec<PersistenceMember>,
}

impl Persistence {
    fn json(&self, manifest: &RecordManifest) -> String {
        let rows = self.members.iter().map(|member| {
            format!(
                "{{\"site\":{},\"status\":{},\"reason\":{},\"point\":{},\"interval\":{},\"point_source\":{}}}",
                js(&member.site), js(member.status), js(&member.reason), rational(member.point),
                interval_json(member.interval),
                member.source.as_ref().map_or_else(|| "null".into(), |cell| observation_json(cell, manifest)),
            )
        }).collect::<Vec<_>>().join(",");
        let ratios = self
            .ratios
            .iter()
            .map(|ratio| rational(Some(*ratio)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"baseline\":\"run_rate_persistence\",\"target\":{},\"pool_from\":{},\"pool_through\":{},\"point_quarter\":{},\"pool_status\":{},\"reason\":{},\"n_ratios\":{},\"excluded_pairs\":{},\"ratio_quantiles\":{},\"ratios\":[{}],\"pool_observations\":[{}],\"rows\":[{}]}}",
            js(&self.target),
            js(&self.from.to_string()),
            js(&self.through.to_string()),
            js(&self.point.to_string()),
            js(self.status),
            js(&self.reason),
            self.excluded_pairs
                .map_or_else(|| "null".into(), |_| self.ratios.len().to_string()),
            self.excluded_pairs
                .map_or_else(|| "null".into(), |n| n.to_string()),
            interval_json(self.quantiles),
            ratios,
            self.pool_observations.join(","),
            rows,
        )
    }
}

/// Multiplication by a printed zero produces [0,0]. A signed point scales the interval with ordered
/// endpoints; only the historical pool's pairs have the frozen positive-number restriction.
fn scaled_interval(
    ratios: &[Rat],
    point: Rat,
    rules: &record::RecordRules,
) -> Result<(Rat, Rat), String> {
    let (a, b) = record::persistence_interval(ratios, point, rules.alpha, rules.estimator)
        .map_err(|e| e.to_string())?;
    match a.checked_cmp(b).map_err(|e| e.to_string())? {
        Ordering::Greater => Ok((b, a)),
        _ => Ok((a, b)),
    }
}

fn persistence(
    loaded: &Loaded,
    profile: &IssuerProfile,
    points: &KeyedClaims,
    point_numbers: &ReportedNumbers,
    baseline: &BaselineRule,
    historical: Result<&History, &Vec<String>>,
) -> Res<Persistence> {
    let BaselineKind::Persistence {
        pool_from,
        pool_through,
        point,
    } = baseline.kind
    else {
        return Err(vec!["persistence adapter needs its registered rule".into()]);
    };
    let target = loaded
        .rules
        .targets
        .iter()
        .find(|target| target.id == baseline.target)
        .ok_or_else(|| vec!["baseline target is undeclared".into()])?;
    let mut out = Persistence {
        target: target.id.clone(),
        from: pool_from,
        through: pool_through,
        point,
        status: "refused",
        reason: String::new(),
        ratios: Vec::new(),
        excluded_pairs: None,
        quantiles: None,
        pool_observations: Vec::new(),
        members: Vec::new(),
    };
    let pool = (|| -> Res<()> {
        let (claims, reported) = historical.map_err(Clone::clone)?;
        let mut series = Vec::new();
        for site in loaded
            .rules
            .sites
            .iter()
            .filter(|site| site.status == SiteStatus::Live)
        {
            let mut values = Vec::new();
            for quarter in pool_from.through(pool_through) {
                let year = u16::try_from(quarter.year)
                    .map_err(|_| vec!["unsupported pool year".into()])?;
                let cell = original(
                    profile,
                    &claims.claims,
                    year,
                    quarter.q,
                    PeriodKind::Quarter(quarter.q),
                    &site.name,
                    target,
                )?;
                let value = cell
                    .map(|cell| numeric(cell, reported))
                    .transpose()?
                    .flatten();
                out.pool_observations.push(format!(
                    "{{\"site\":{},\"quarter\":{},\"displayed_value\":{},\"evidence\":{}}}",
                    js(&site.name),
                    js(&quarter.to_string()),
                    rational(value),
                    cell.map_or_else(
                        || "null".into(),
                        |cell| observation_json(cell, &loaded.manifest)
                    ),
                ));
                values.push(value);
            }
            series.push(values);
        }
        let (ratios, left_out) = record::pooled_ratios(&series).map_err(|e| vec![e.to_string()])?;
        out.ratios = ratios;
        out.excluded_pairs = Some(left_out);
        if out.ratios.is_empty() {
            return Err(vec![
                "no eligible positive consecutive-quarter pairs in the frozen pool".into(),
            ]);
        }
        out.quantiles = Some(
            record::ratio_quantiles(&out.ratios, loaded.rules.alpha, loaded.rules.estimator)
                .map_err(|e| vec![e.to_string()])?,
        );
        Ok(())
    })();
    match pool {
        Ok(()) => {
            out.status = "available";
            out.reason = baseline.rule.clone();
        }
        Err(errors) => out.reason = errors.join("; "),
    }
    for site in &loaded.rules.sites {
        let mut member = PersistenceMember {
            site: site.name.clone(),
            status: "not_available",
            reason: String::new(),
            point: None,
            interval: None,
            source: None,
        };
        if site.status == SiteStatus::NotPublished {
            member.status = "not_published";
            member.reason = site
                .reason
                .clone()
                .unwrap_or_else(|| "excluded by frozen site universe".into());
        } else {
            let result = (|| -> Res<()> {
                let year =
                    u16::try_from(point.year).map_err(|_| vec!["unsupported point year".into()])?;
                let cell = original(
                    profile,
                    &points.claims,
                    year,
                    point.q,
                    PeriodKind::Quarter(point.q),
                    &site.name,
                    target,
                )?;
                member.source = cell.cloned();
                member.point = cell
                    .map(|cell| numeric(cell, point_numbers))
                    .transpose()?
                    .flatten();
                if out.quantiles.is_none() {
                    return Err(vec![format!(
                        "fixed persistence pool unavailable: {}",
                        out.reason
                    )]);
                }
                let value = member.point.ok_or_else(|| vec![
                    "fixed original point release has no displayed number for this frozen member".into()
                ])?;
                member.interval =
                    Some(scaled_interval(&out.ratios, value, &loaded.rules).map_err(|e| vec![e])?);
                Ok(())
            })();
            match result {
                Ok(()) => {
                    member.status = "available";
                    member.reason = baseline.rule.clone();
                }
                Err(errors) => member.reason = errors.join("; "),
            }
        }
        out.members.push(member);
    }
    Ok(out)
}

fn comparison_json(target: &TargetComparison, status: &str, errors: &[String]) -> String {
    let exclusions = target.exclusions.iter().map(|row| format!(
        "{{\"site\":{},\"missing_actual\":{},\"missing_issuer_guidance\":{},\"missing_run_rate_persistence\":{}}}",
        js(&row.site), row.missing_actual, row.missing_issuer_guidance, row.missing_run_rate_persistence,
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"target\":{},\"status\":{},\"errors\":{},\"n_frozen\":{},\"n_common\":{},\"common_members\":{},\"exclusions\":[{}],\"mean_forecast_score\":{},\"mean_issuer_guidance_score\":{},\"mean_run_rate_persistence_score\":{},\"forecast_minus_issuer_guidance\":{},\"forecast_minus_run_rate_persistence\":{}}}",
        js(&target.target),
        js(status),
        names(errors),
        target.n_frozen,
        target.common_members.len(),
        names(&target.common_members),
        exclusions,
        rational(target.mean_forecast_score),
        rational(target.mean_issuer_guidance_score),
        rational(target.mean_run_rate_persistence_score),
        rational(target.forecast_minus_issuer_guidance),
        rational(target.forecast_minus_run_rate_persistence),
    )
}

/// An overflow in one optional comparison must not discard the model scores or another target.
/// When exact arithmetic refuses, membership is still determined from the original evidence and listed.
fn comparisons(loaded: &Loaded, outcomes: &[OutcomeRow], rows: &[BenchmarkRow]) -> Vec<String> {
    loaded
        .rules
        .targets
        .iter()
        .map(|target| {
            let mut rules = loaded.rules.clone();
            rules.targets.retain(|row| row.id == target.id);
            rules.baselines.retain(|row| row.target == target.id);
            let forecasts: Vec<_> = loaded
                .forecasts
                .iter()
                .filter(|row| row.target == target.id)
                .cloned()
                .collect();
            let actuals: Vec<_> = outcomes
                .iter()
                .filter(|row| row.target == target.id)
                .cloned()
                .collect();
            let benchmarks: Vec<_> = rows
                .iter()
                .filter(|row| row.target == target.id)
                .cloned()
                .collect();
            match scoring::compare(&rules, &forecasts, &actuals, &benchmarks) {
                Ok(targets) => comparison_json(&targets[0], "assessed", &[]),
                Err(error) => {
                    let mut population = TargetComparison {
                        target: target.id.clone(),
                        n_frozen: 0,
                        common_members: Vec::new(),
                        exclusions: Vec::new(),
                        mean_forecast_score: None,
                        mean_issuer_guidance_score: None,
                        mean_run_rate_persistence_score: None,
                        forecast_minus_issuer_guidance: None,
                        forecast_minus_run_rate_persistence: None,
                    };
                    for site in rules
                        .sites
                        .iter()
                        .filter(|site| site.status == SiteStatus::Live)
                    {
                        population.n_frozen += 1;
                        let actual = actuals.iter().find(|row| row.site == site.name);
                        let benchmark = benchmarks.iter().find(|row| row.site == site.name);
                        let exclusion = ComparisonExclusion {
                            site: site.name.clone(),
                            missing_actual: actual.and_then(|row| row.value).is_none(),
                            missing_issuer_guidance: benchmark
                                .and_then(|row| row.issuer_guidance)
                                .is_none(),
                            missing_run_rate_persistence: benchmark
                                .and_then(|row| row.run_rate_persistence)
                                .is_none(),
                        };
                        if exclusion.missing_actual
                            || exclusion.missing_issuer_guidance
                            || exclusion.missing_run_rate_persistence
                        {
                            population.exclusions.push(exclusion);
                        } else {
                            population.common_members.push(site.name.clone());
                        }
                    }
                    comparison_json(&population, "refused", &[error.to_string()])
                }
            }
        })
        .collect()
}

pub(super) fn assess(
    root: &Path,
    loaded: &Loaded,
    profile: &IssuerProfile,
    outcomes: &[OutcomeRow],
    points: &KeyedClaims,
    point_numbers: &ReportedNumbers,
) -> Res<String> {
    let year = super::year(&loaded.rules)?;
    let date_bound = guidance_date_bound(loaded.manifest.information_cutoff_utc);
    let transcripts = load_transcription_prefixes(profile, root, &["guidance_"], Some(&date_bound))
        .map_err(|e| vec![e.to_string()]);
    let bound_guidance = transcripts
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|report| {
            guidance::bind_rows(
                &loaded.rules,
                year,
                report.as_ref(),
                &points.claims,
                |cell| {
                    numeric(cell, point_numbers)
                        .map_err(|errors| errors.join("; "))?
                        .ok_or_else(|| {
                            "original nine-month cell is not displayed as a number".into()
                        })
                },
                |evidence| guidance_timing(&evidence.document, &loaded.manifest).map(|_| ()),
            )
        });
    let (guidance, guidance_errors) = match bound_guidance {
        Ok(rows) => (rows, Vec::new()),
        Err(errors) => (missing_guidance(loaded, &errors.join("; ")), errors),
    };
    let mut histories = BTreeMap::new();
    let mut persistence_rows = Vec::new();
    for baseline in &loaded.rules.baselines {
        if let BaselineKind::Persistence {
            pool_from,
            pool_through,
            ..
        } = baseline.kind
        {
            let history = histories
                .entry((pool_from, pool_through))
                .or_insert_with(|| history(root, profile, pool_from, pool_through));
            persistence_rows.push(persistence(
                loaded,
                profile,
                points,
                point_numbers,
                baseline,
                history.as_ref(),
            )?);
        }
    }
    let rows: Vec<_> = loaded
        .forecasts
        .iter()
        .map(|forecast| BenchmarkRow {
            site: forecast.site.clone(),
            target: forecast.target.clone(),
            issuer_guidance: guidance
                .iter()
                .find(|row| row.site == forecast.site && row.target == forecast.target)
                .and_then(|row| row.interval),
            run_rate_persistence: persistence_rows
                .iter()
                .find(|row| row.target == forecast.target)
                .and_then(|row| row.members.iter().find(|row| row.site == forecast.site))
                .and_then(|row| row.interval),
        })
        .collect();
    let comparisons = comparisons(loaded, outcomes, &rows);
    Ok(format!(
        "{{\"guidance_information_cutoff_utc\":{},\"guidance_document_date_bound\":{},\"guidance_errors\":{},\"guidance_nominal_probability\":null,\"score_alpha\":{},\"information_sets\":\"guidance is strictly pre-cut; the frozen original Q3 observations and historical persistence pool are separate registered benchmark inputs, with manifest membership disclosed\",\"q3_release_gate_utc\":{},\"q3_release_gate_after_cutoff\":{},\"release_gate_basis\":\"frozen_rules_schedule_not_observed_publication_timestamp\",\"same_information_set_claim\":false,\"skill_claim\":false,\"calibration_claim\":false,\"guidance\":[{}],\"persistence\":[{}],\"comparisons\":[{}]}}",
        js(&format_utc(loaded.manifest.information_cutoff_utc)),
        js(&date_bound),
        names(&guidance_errors),
        rational(Some(loaded.rules.alpha)),
        js(&format_utc(loaded.rules.dates.nowcast_from)),
        loaded.rules.dates.nowcast_from > loaded.manifest.information_cutoff_utc,
        guidance
            .iter()
            .map(|row| guidance_json(row, &loaded.manifest))
            .collect::<Vec<_>>()
            .join(","),
        persistence_rows
            .iter()
            .map(|row| row.json(&loaded.manifest))
            .collect::<Vec<_>>()
            .join(","),
        comparisons.join(","),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    };
    use fmine_public::LoadedSource;
    use fmine_public::sources::RegisteredSource;

    fn loaded() -> Loaded {
        let mut rules =
            record::parse_rules(include_str!("../../../../../forecasts/fy2026/rules.toml"))
                .unwrap();
        rules.issuer = fmine_public::synthetic::SYNTHETIC_ISSUER.into();
        rules.sites.truncate(1);
        rules.sites[0].name = "Example Mine".into();
        for baseline in &mut rules.baselines {
            if let BaselineKind::Persistence {
                pool_from,
                pool_through,
                ..
            } = &mut baseline.kind
            {
                *pool_from = Quarter::parse("2024Q1").unwrap();
                *pool_through = Quarter::parse("2024Q2").unwrap();
            }
        }
        let forecasts = rules
            .targets
            .iter()
            .map(|target| record::ForecastRow {
                site: "Example Mine".into(),
                target: target.id.clone(),
                point: Rat::from_integer(100),
                lo: Rat::from_integer(90),
                hi: Rat::from_integer(110),
            })
            .collect();
        Loaded {
            manifest: RecordManifest {
                record_id: rules.record_id.clone(),
                author_role: "invented-test-role".into(),
                information_cutoff_utc: parse_utc("2026-11-01T12:00:00Z").unwrap(),
                label: record::Label::Forecast,
                contact: record::Contact::PreContact,
                attestation: "synthetic test, not a registered forecast".into(),
                rules_sha256: [0; 32],
                inputs: Vec::new(),
            },
            dates: rules.dates,
            digest: String::new(),
            rules_digest: String::new(),
            registered: Vec::new(),
            rules,
            forecasts,
        }
    }

    fn document(date: &str) -> CheckedDocument {
        CheckedDocument {
            file: "invented.pdf".into(),
            kind: "mda".into(),
            published_at: date.into(),
            published_at_basis: "synthetic test".into(),
            sha256: Digest32([7; 32]).to_hex(),
        }
    }

    #[test]
    fn strict_guidance_cut_keeps_same_day_uncertainty_and_names_declared_retrieval() {
        let mut manifest = loaded().manifest;
        let same_day = document("2026-11-01");
        assert_eq!(
            guidance_timing(&document("2026-10-31"), &manifest).unwrap(),
            "registered_publication_date_before_cutoff_day"
        );
        assert!(guidance_timing(&same_day, &manifest).is_err());
        assert!(guidance_timing(&document("2026-11-02"), &manifest).is_err());
        manifest.inputs.push(record::InputDoc {
            doc_id: same_day.file.clone(),
            url: "https://example.invalid/invented.pdf".into(),
            publication_day: manifest.information_cutoff_utc.div_euclid(86_400),
            retrieved_utc: manifest.information_cutoff_utc - 1,
            bytes: 123,
            sha256: [7; 32],
            line: 1,
        });
        assert_eq!(
            guidance_timing(&same_day, &manifest).unwrap(),
            "manifest_declared_retrieval_before_cutoff"
        );
        manifest.inputs[0].retrieved_utc = manifest.information_cutoff_utc;
        assert!(guidance_timing(&same_day, &manifest).is_err());
        manifest.inputs[0].retrieved_utc -= 1;
        manifest.inputs[0].sha256[0] ^= 1;
        assert!(guidance_timing(&same_day, &manifest).is_err());
        assert_eq!(
            guidance_date_bound(parse_utc("2026-11-01T00:00:00Z").unwrap()),
            "2026-10-31"
        );
        assert_eq!(
            guidance_date_bound(parse_utc("2026-11-01T00:00:01Z").unwrap()),
            "2026-11-01"
        );
    }

    /// Source bytes are invented. Their display rounds the stored tenths to integers, which independently
    /// distinguishes the registered printed-number rule from multiplication of hidden stored precision.
    fn workbook(profile: &mut IssuerProfile, release: &str, raw: &str) -> History {
        let quarter = Quarter::parse(release).unwrap();
        let text = |s: &str| MinimalCell::Text(s.into());
        let number = |s: &str| MinimalCell::Styled {
            raw: s.into(),
            xf: 0,
        };
        let end =
            ["March 31", "June 30", "September 30", "December 31"][usize::from(quarter.q - 1)];
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "QTD".into(),
                hidden: false,
                cells: vec![
                    (
                        "A1".into(),
                        text(&format!("For the three months ended {end},")),
                    ),
                    ("B1".into(), number(&quarter.year.to_string())),
                    ("B2".into(), text("Gold produced (000s oz) (100%)")),
                    ("C2".into(), text("All-in sustaining costs ($/oz)")),
                    ("A3".into(), text("Example Mine (60%)")),
                    ("B3".into(), number(raw)),
                    ("C3".into(), number("1000")),
                ],
            }],
            Some(&MinimalStyles {
                num_fmts: Vec::new(),
                xfs: vec![1],
            }),
        );
        let file = format!("invented_{release}.xlsx");
        let workbook = Workbook::from_bytes(&file, &bytes).unwrap();
        let read = fmine_public::mine_stats::read_mine_stats(&workbook, release, None).unwrap();
        assert!(read.refusals.is_empty());
        profile.sources.push(RegisteredSource {
            file_name: file.clone(),
            kind: SourceKind::MineStats,
            quarter: release.into(),
            url: format!("https://example.invalid/{file}"),
            url_basis: "synthetic test".into(),
            bytes: bytes.len() as u64,
            sha256_hex: Digest32(workbook.sha256).to_hex(),
        });
        let source = LoadedSource {
            file_name: file.clone(),
            kind: SourceKind::MineStats,
            quarter: release.into(),
            url: format!("https://example.invalid/{file}"),
            url_basis: "synthetic test".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        };
        let numbers = super::super::reported_numbers(std::slice::from_ref(&source));
        let claims =
            fmine_public::key::mine_stats::build_claims(profile, &[source], &[(file, read)]);
        assert!(claims.refusals.is_empty(), "{:?}", claims.refusals);
        (claims, numbers)
    }

    fn profile() -> IssuerProfile {
        let mut profile = fmine_public::synthetic::synthetic_profile();
        profile.sources.clear();
        profile.subject_statuses.clear();
        profile
    }

    #[test]
    fn fixed_persistence_uses_original_displayed_numbers_outside_forecast_manifest() {
        let loaded = loaded();
        let mut profile = profile();
        let mut historical = workbook(&mut profile, "2024Q1", "100.4");
        let second = workbook(&mut profile, "2024Q2", "200.4");
        historical.0.claims.extend(second.0.claims);
        historical.0.digest = fmine_public::key::claims_digest(&historical.0.claims);
        historical.1.extend(second.1);
        let baseline = loaded
            .rules
            .baselines
            .iter()
            .find(|baseline| {
                baseline.target == "production"
                    && matches!(baseline.kind, BaselineKind::Persistence { .. })
            })
            .unwrap();
        for (printed, expected) in [("10.4", 20), ("0", 0), ("-2.4", -4)] {
            let mut profile = profile.clone();
            let point = workbook(&mut profile, "2026Q3", printed);
            let result = persistence(
                &loaded,
                &profile,
                &point.0,
                &point.1,
                baseline,
                Ok(&historical),
            )
            .unwrap();
            assert_eq!(result.ratios, [Rat::from_integer(2)]);
            assert_eq!(result.excluded_pairs, Some(0));
            assert_eq!(
                result.members[0].interval,
                Some((Rat::from_integer(expected), Rat::from_integer(expected)))
            );
            let json = result.json(&loaded.manifest);
            assert!(json.contains("\"in_forecast_manifest\":false"));
            assert!(json.contains("\"stored\":\"100.4\""));
        }
        let interval = scaled_interval(
            &[Rat::new(1, 2).unwrap(), Rat::from_integer(2)],
            Rat::from_integer(-10),
            &loaded.rules,
        )
        .unwrap();
        assert_eq!(interval, (Rat::from_integer(-20), Rat::from_integer(-5)));
    }

    #[test]
    fn missing_history_and_unreadable_registered_history_do_not_erase_model_scores() {
        let loaded = loaded();
        let mut profile = profile();
        let root = std::env::temp_dir().join("fmine-invented-unavailable-benchmark-corpus");
        let points = fmine_public::key::mine_stats::build_claims(&profile, &[], &[]);
        let numbers = BTreeMap::new();
        let outcomes: Vec<_> = loaded
            .forecasts
            .iter()
            .map(|forecast| OutcomeRow {
                site: forecast.site.clone(),
                target: forecast.target.clone(),
                value: Some(Rat::from_integer(120)),
            })
            .collect();
        let model = scoring::assess(&loaded.rules, &loaded.forecasts, &outcomes).unwrap();
        assert_eq!(
            model.targets[0].observed_mean_interval_score,
            Some(Rat::from_integer(120))
        );
        let absent = assess(&root, &loaded, &profile, &outcomes, &points, &numbers).unwrap();
        assert!(
            absent.contains("fixed persistence pool requires one original registration; found 0")
        );
        assert!(absent.contains("\"n_common\":0"));
        assert!(absent.contains("\"missing_actual\":false"));
        for quarter in ["2024Q1", "2024Q2"] {
            profile.sources.push(RegisteredSource {
                file_name: format!("invented_missing_{quarter}.xlsx"),
                kind: SourceKind::MineStats,
                quarter: quarter.into(),
                url: format!("https://example.invalid/{quarter}.xlsx"),
                url_basis: "synthetic test".into(),
                bytes: 123,
                sha256_hex: Digest32([8; 32]).to_hex(),
            });
        }
        let unreadable = assess(&root, &loaded, &profile, &outcomes, &points, &numbers).unwrap();
        assert!(unreadable.contains("invented_missing_2024Q1.xlsx"));
        assert!(unreadable.contains("\"pool_status\":\"refused\""));
        assert!(unreadable.contains("\"n_ratios\":null"));
        assert!(unreadable.contains("\"missing_actual\":false"));
        assert_eq!(
            scoring::assess(&loaded.rules, &loaded.forecasts, &outcomes).unwrap(),
            model
        );
    }
    #[test]
    fn comparator_overflow_preserves_population_and_the_other_target() {
        let loaded = loaded();
        let actuals: Vec<_> = loaded
            .forecasts
            .iter()
            .map(|forecast| OutcomeRow {
                site: forecast.site.clone(),
                target: forecast.target.clone(),
                value: Some(Rat::from_integer(120)),
            })
            .collect();
        let normal = (Rat::from_integer(90), Rat::from_integer(110));
        let huge = (
            Rat::from_integer(i128::MAX - 1),
            Rat::from_integer(i128::MAX),
        );
        let benchmarks: Vec<_> = loaded
            .forecasts
            .iter()
            .map(|forecast| BenchmarkRow {
                site: forecast.site.clone(),
                target: forecast.target.clone(),
                issuer_guidance: Some(if forecast.target == "production" {
                    huge
                } else {
                    normal
                }),
                run_rate_persistence: Some(normal),
            })
            .collect();
        let rows = comparisons(&loaded, &actuals, &benchmarks);
        assert!(rows[0].contains("\"status\":\"refused\""));
        assert!(rows[0].contains("\"common_members\":[\"Example Mine\"]"));
        assert!(rows[0].contains("\"mean_forecast_score\":null"));
        assert!(rows[1].contains("\"status\":\"assessed\""));
        assert!(rows[1].contains("\"mean_forecast_score\":\"120\""));
        assert!(rows[1].contains("\"forecast_minus_issuer_guidance\":\"0\""));
        assert_eq!(
            scoring::assess(&loaded.rules, &loaded.forecasts, &actuals)
                .unwrap()
                .targets[0]
                .observed_mean_interval_score,
            Some(Rat::from_integer(120))
        );
    }

    #[test]
    fn uncertain_latest_guidance_is_not_replaced_and_admitted_printed_ranges_remain_unchanged() {
        use fmine_public::transcribe::TranscriptionReport;
        use fmine_registry::transcriptions::{Accepted, Agreement, Locator, parse_token};
        let mut loaded = loaded();
        let mut latest = document("2026-11-01");
        latest.file = "latest.pdf".into();
        let earlier = document("2026-10-31");
        let reading = |document: &CheckedDocument, token: &str| Accepted {
            locator: Locator {
                doc: document.file.clone(),
                page: 1,
                table: "Invented".into(),
                row: "Example Mine".into(),
                col: "AISC".into(),
            },
            doc_sha256: document.sha256.clone(),
            token: token.into(),
            value: parse_token(token).unwrap(),
            quantity: "guidance_example_mine_aisc_usd_per_oz".into(),
            unit: "usd_per_oz".into(),
            basis: "attributable:3/5;aisc=issuer-reported-per-sold-oz".into(),
            period: "2026FY".into(),
            readers: vec!["reader-a".into(), "reader-b".into()],
        };
        let report = TranscriptionReport {
            registry: "invented-test".into(),
            documents: vec![earlier.clone(), latest.clone()],
            entries: 4,
            agreement: Agreement {
                accepted: vec![reading(&earlier, "900 - 1100"), reading(&latest, "-100")],
                ..Agreement::default()
            },
            tie: None,
        };
        let bind = |loaded: &Loaded| {
            guidance::bind_rows(
                &loaded.rules,
                2026,
                Some(&report),
                &[],
                |_| Err("no nine-month observation in this cost-only control".into()),
                |evidence| guidance_timing(&evidence.document, &loaded.manifest).map(|_| ()),
            )
            .unwrap()
        };
        let rows = bind(&loaded);
        let cost = rows.iter().find(|row| row.target == "unit_cost").unwrap();
        assert!(cost.interval.is_none());
        assert_eq!(cost.guidance[0].document.file, latest.file);
        assert!(cost.reason.contains("same-day"));
        loaded.manifest.inputs.push(record::InputDoc {
            doc_id: latest.file.clone(),
            url: "https://example.invalid/latest.pdf".into(),
            publication_day: loaded.manifest.information_cutoff_utc.div_euclid(86_400),
            retrieved_utc: loaded.manifest.information_cutoff_utc - 1,
            bytes: 123,
            sha256: [7; 32],
            line: 1,
        });
        let rows = bind(&loaded);
        let cost = rows.iter().find(|row| row.target == "unit_cost").unwrap();
        assert_eq!(
            cost.interval,
            Some((Rat::from_integer(-100), Rat::from_integer(-100)))
        );
        let json = guidance_json(cost, &loaded.manifest);
        assert!(json.contains("\"nominal_probability\":null"));
        assert!(json.contains("manifest_declared_retrieval_before_cutoff"));
    }
}
