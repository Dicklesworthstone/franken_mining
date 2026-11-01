//! Page-cited technical production scenarios (BD-342). A plan or nameplate is not a physical maximum.
//! All three inputs must explicitly apply to the exact remaining months, on a full-mine dry metric-tonne
//! basis. Annual rates, life-of-mine averages and reserve grades are not silently repurposed or prorated.

use fmine_metrics::capacity::{
    self as physical, CapacityLimits, CapacityReport, CapacityVerdict, PhysicalKind, PhysicalLimit,
};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechnicalScenarioKind {
    DeclaredUpperBound,
    Plan,
    Nameplate,
}

impl TechnicalScenarioKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredUpperBound => "declared_upper_bound",
            Self::Plan => "plan",
            Self::Nameplate => "nameplate",
        }
    }

    #[must_use]
    pub const fn interpretation(self) -> &'static str {
        match self {
            Self::DeclaredUpperBound => {
                "Upper limits explicitly stated for the remaining period. Exclusion is conditional on those limits, not a claim that the mine is physically incapable of meeting guidance."
            }
            Self::Plan => {
                "Stated plan inputs are conditional scenario parameters, not physical upper bounds. Not excluded supplies no feasible operating plan or joint production-cost witness."
            }
            Self::Nameplate => {
                "Stated nameplate inputs are conditional scenario parameters, not demonstrated physical maxima. Not excluded supplies no feasible operating plan or joint production-cost witness."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TechnicalComparison {
    pub kind: TechnicalScenarioKind,
    pub document: CheckedDocument,
    pub sources: Vec<Accepted>,
    pub limits: CapacityLimits,
    pub result: CapacityReport,
}

#[derive(Debug, Clone)]
pub enum TechnicalAssessment {
    Compared(Box<TechnicalComparison>),
    Indeterminate { reason: Refusal },
}

impl TechnicalAssessment {
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::Compared(_) => "T2",
            Self::Indeterminate { .. } => "T3",
        }
    }
}

pub(super) fn quantities(site: &str) -> [String; 3] {
    let stem = format!("technical_{}", site.replace('-', "_"));
    [
        "remaining_dry_feed_mt",
        "head_grade_g_per_t",
        "recovery_percent",
    ]
    .map(|suffix| format!("{stem}_{suffix}"))
}

fn support(p: Period) -> Result<String> {
    let suffix = match (p.first_month, p.last_month) {
        (4, 12) => "Q2-Q4",
        (7, 12) => "H2",
        (10, 12) => "Q4",
        _ => {
            return Err(refuse(
                RefusalCode::SupportMismatch102,
                "technical inputs require the exact remaining calendar-quarter support",
            ));
        }
    };
    Ok(format!("{}{suffix}", p.year))
}

fn kind(basis: &str) -> Result<TechnicalScenarioKind> {
    match basis {
        "100%;scenario=declared_upper_bound" => Ok(TechnicalScenarioKind::DeclaredUpperBound),
        "100%;scenario=plan" => Ok(TechnicalScenarioKind::Plan),
        "100%;scenario=nameplate" => Ok(TechnicalScenarioKind::Nameplate),
        _ => Err(refuse(
            RefusalCode::BasisMismatch105,
            "technical basis must declare full-mine 100% and scenario=declared_upper_bound, plan or nameplate; no attributable physical factors or inferred scenario kind",
        )),
    }
}

fn rows<'a>(
    report: &'a TranscriptionReport,
    site: &str,
    as_of: &str,
) -> Result<([&'a Accepted; 3], &'a CheckedDocument)> {
    if !is_date(as_of) {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "technical evidence cut needs YYYY-MM-DD",
        ));
    }
    if !report.agreement.is_consistent() {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "technical-input readers disagree",
        ));
    }
    let keys = quantities(site);
    let candidates: Vec<_> = report
        .documents
        .iter()
        .filter(|d| {
            is_date(&d.published_at)
                && d.published_at.as_str() <= as_of
                && (report
                    .agreement
                    .accepted
                    .iter()
                    .any(|a| a.locator.doc == d.file && keys.contains(&a.quantity))
                    || report
                        .agreement
                        .single_reader
                        .iter()
                        .any(|(at, _)| at.doc == d.file))
        })
        .collect();
    let latest = candidates.iter().map(|d| d.published_at.as_str()).max()
        .ok_or_else(|| refuse(RefusalCode::NoClaimInDomain201, format!("no registered technical production inputs for {site} published by {as_of}; a same-period dry-feed, grade and recovery scenario is required")))?;
    let latest: Vec<_> = candidates
        .into_iter()
        .filter(|d| d.published_at == latest)
        .collect();
    let [doc] = latest.as_slice() else {
        return Err(refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "multiple technical-input documents have the latest publication date",
        ));
    };
    let doc = *doc;
    if doc.kind != "technical_report" {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!(
                "{} is registered as {}; T2 needs kind technical_report, not a deck or another filing kind",
                doc.file, doc.kind
            ),
        ));
    }
    if report
        .agreement
        .single_reader
        .iter()
        .any(|(at, _)| at.doc == doc.file)
    {
        return Err(refuse(
            RefusalCode::NotObservable202,
            format!(
                "{} has pending technical-input readers; older complete inputs are not substituted",
                doc.file
            ),
        ));
    }
    let row = |key: &str| -> Result<&Accepted> {
        let matched: Vec<_> = report
            .agreement
            .accepted
            .iter()
            .filter(|a| a.locator.doc == doc.file && a.quantity == key)
            .collect();
        let [a] = matched.as_slice() else {
            return Err(refuse(
                RefusalCode::NotObservable202,
                format!(
                    "latest technical report {} needs exactly one agreed {key}",
                    doc.file
                ),
            ));
        };
        if a.doc_sha256 != doc.sha256 || a.readers.iter().collect::<BTreeSet<_>>().len() < 2 {
            return Err(refuse(
                RefusalCode::NotObservable202,
                format!(
                    "{} lacks two agreeing readers of the checked document bytes",
                    a.locator
                ),
            ));
        }
        Ok(*a)
    };
    Ok(([row(&keys[0])?, row(&keys[1])?, row(&keys[2])?], doc))
}

fn limit(
    a: &Accepted,
    required: &Quantity,
    scenario: TechnicalScenarioKind,
    physical_kind: PhysicalKind,
    unit: &str,
    synthetic: bool,
) -> Result<Claim<PhysicalLimit>> {
    let expected_support = support(required.period)?;
    if a.period != expected_support {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            format!(
                "{} covers {}, but the remaining requirement covers {expected_support}; annual rates, life-of-mine averages and other periods are not prorated",
                a.locator, a.period
            ),
        ));
    }
    if a.unit != unit {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!(
                "{} needs explicit {unit}, found {}; no wet/dry, annual-rate or tonne conversion is implicit",
                a.locator, a.unit
            ),
        ));
    }
    if kind(&a.basis)? != scenario {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "technical feed, grade and recovery must declare one common scenario kind",
        ));
    }
    let rounded =
        scenario != TechnicalScenarioKind::DeclaredUpperBound && matches!(a.value, Token::Point(_));
    let mut interval = token_interval(a.value, rounded)?;
    if physical_kind == PhysicalKind::RecoveryFraction {
        interval.lo = exact(interval.lo.checked_div(Rat::from_integer(100)))?;
        interval.hi = exact(interval.hi.checked_div(Rat::from_integer(100)))?;
    }
    let value = PhysicalLimit {
        subject: required.subject.clone(),
        period: required.period,
        ownership: Ownership::Full,
        kind: physical_kind,
        interval,
    };
    let tag = value.canonical_tag();
    let assumed = synthetic || scenario != TechnicalScenarioKind::DeclaredUpperBound;
    Ok(Claim::new(
        format!("guidance.technical.{}.{}", scenario.as_str(), a.quantity),
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        if physical_kind == PhysicalKind::RemainingDryFeedMt {
            AdditivityClass::ExtensiveAdditive
        } else {
            AdditivityClass::IntensiveWeighted
        },
        if synthetic {
            EvidenceClass::Assumed
        } else {
            EvidenceClass::ISSUER
        },
        if assumed {
            EpistemicStatus::KnownUnknown(KuKind::Parametric)
        } else {
            EpistemicStatus::KnownKnown
        },
        if assumed {
            KnowledgeState::Estimated
        } else {
            KnowledgeState::Known
        },
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            &a.locator.doc,
            sha_bytes(&a.doc_sha256)?,
            a.locator.to_string(),
        )],
    ))
}

fn compare(
    required: &Claim<Quantity>,
    report: &TranscriptionReport,
    site: &str,
    as_of: &str,
    synthetic: bool,
) -> Result<TechnicalComparison> {
    let (selected, document) = rows(report, site, as_of)?;
    let scenario = kind(&selected[0].basis)?;
    let limits = CapacityLimits {
        feed: limit(
            selected[0],
            &required.value,
            scenario,
            PhysicalKind::RemainingDryFeedMt,
            "dry_mt",
            synthetic,
        )?,
        grade: limit(
            selected[1],
            &required.value,
            scenario,
            PhysicalKind::HeadGradeGPerT,
            "g_per_dry_t",
            synthetic,
        )?,
        recovery: limit(
            selected[2],
            &required.value,
            scenario,
            PhysicalKind::RecoveryFraction,
            "percent",
            synthetic,
        )?,
    };
    let result = physical::compare_production(required, &limits)?;
    Ok(TechnicalComparison {
        kind: scenario,
        document: document.clone(),
        sources: selected.into_iter().cloned().collect(),
        limits,
        result,
    })
}

pub(super) fn assess(
    required: &Claim<Quantity>,
    source: Result<Option<&TranscriptionReport>>,
    site: &str,
    as_of: &str,
    synthetic: bool,
) -> TechnicalAssessment {
    let result = source
        .and_then(|report| {
            report.ok_or_else(|| {
                refuse(
                    RefusalCode::NoClaimInDomain201,
                    "no registered technical production scenario was supplied",
                )
            })
        })
        .and_then(|report| compare(required, report, site, as_of, synthetic));
    match result {
        Ok(comparison) => TechnicalAssessment::Compared(Box::new(comparison)),
        Err(reason) => TechnicalAssessment::Indeterminate { reason },
    }
}

fn physical_json(c: &Claim<PhysicalLimit>) -> String {
    let p = &c.value;
    let rat = |v: Rat| js(&format!("{}/{}", v.num(), v.den()));
    format!(
        "{{{},\"subject\":{},\"year\":{},\"first_month\":{},\"last_month\":{},\"ownership_basis\":\"100_percent\",\"kind\":{},\"interval\":{{\"lo\":{},\"hi\":{},\"closed\":true}}}}",
        claim_fields(c),
        js(&p.subject),
        p.period.year,
        p.period.first_month,
        p.period.last_month,
        js(match p.kind {
            PhysicalKind::RemainingDryFeedMt => "remaining_dry_metric_feed_mt",
            PhysicalKind::HeadGradeGPerT => "gold_head_grade_g_per_dry_metric_t",
            PhysicalKind::RecoveryFraction => "gold_recovery_fraction",
        }),
        rat(p.interval.lo),
        rat(p.interval.hi)
    )
}

pub(super) fn to_json(assessment: &TechnicalAssessment) -> String {
    let c = match assessment {
        TechnicalAssessment::Compared(c) => c,
        TechnicalAssessment::Indeterminate { reason } => {
            return format!(
                "{{\"tier\":\"T3\",\"scope\":\"production_only\",\"status\":\"indeterminate\",\"reason\":{{\"code\":{},\"what\":{},\"fix\":{}}},\"physical_impossibility_claim\":false,\"joint_cost_feasibility_claim\":false}}",
                js(reason.code),
                js(&reason.what),
                js(reason.fix)
            );
        }
    };
    let status = match c.result.verdict {
        CapacityVerdict::AlreadySatisfied => "already_satisfied".to_string(),
        CapacityVerdict::ExceedsStatedProductionCeiling => {
            format!("exceeds_stated_{}_scenario", c.kind.as_str())
        }
        CapacityVerdict::NotExcluded => {
            format!("not_excluded_by_stated_{}_scenario", c.kind.as_str())
        }
    };
    let rows = c.sources.iter().map(|a| format!("{{\"locator\":{},\"sha256\":{},\"token\":{},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader_count\":{}}}", js(&a.locator.to_string()), js(&a.doc_sha256), js(&a.token), js(&a.quantity), js(&a.unit), js(&a.basis), js(&a.period), a.readers.len())).collect::<Vec<_>>().join(",");
    format!(
        "{{\"tier\":\"T2\",\"scope\":\"production_only\",\"status\":{},\"scenario_kind\":{},\"interpretation\":{},\"applicability\":\"Exact remaining months only; annual rates and life-of-mine averages are not prorated.\",\"rounding\":{},\"conversion\":\"Explicit recovery percent divided by 100; all physical factors are full-mine dry metric-tonne quantities; ownership scales production only.\",\"document\":{{\"file\":{},\"kind\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{}}},\"sources\":[{}],\"limits\":{{\"feed\":{},\"grade\":{},\"recovery\":{}}},\"maximum_production\":{},\"optimistic_required_grade\":{},\"physical_impossibility_claim\":false,\"joint_cost_feasibility_claim\":false}}",
        js(&status),
        js(c.kind.as_str()),
        js(c.kind.interpretation()),
        js(if c.kind == TechnicalScenarioKind::DeclaredUpperBound {
            "Stated upper-limit values and ranges are exact declared boundaries."
        } else {
            "Printed plan/nameplate points retain half-last-place rounding; stated ranges retain their exact endpoints. The upper product is a conditional scenario bound, not a physical maximum."
        }),
        js(&c.document.file),
        js(&c.document.kind),
        js(&c.document.sha256),
        js(&c.document.published_at),
        js(&c.document.published_at_basis),
        rows,
        physical_json(&c.limits.feed),
        physical_json(&c.limits.grade),
        physical_json(&c.limits.recovery),
        quantity_json(&c.result.maximum_production),
        physical_json(&c.result.optimistic_required_grade)
    )
}

pub(super) fn synthetic_report() -> Result<TranscriptionReport> {
    let digest = sha256(b"Invented technical report: Example Mine, 2030H2, full-mine upper limits: dry feed 10 Mt, gold head grade 2 g/t, gold recovery 90 percent.");
    let doc = CheckedDocument {
        file: "synthetic-technical-report.txt".into(),
        kind: "technical_report".into(),
        published_at: "2030-06-30".into(),
        published_at_basis: "DESIGN: invented publication date".into(),
        sha256: digest.to_hex(),
    };
    let mut accepted = Vec::new();
    for ((quantity, token), unit) in quantities("example-mine")
        .into_iter()
        .zip(["10", "2", "90"])
        .zip(["dry_mt", "g_per_dry_t", "percent"])
    {
        accepted.push(Accepted {
            locator: Locator {
                doc: doc.file.clone(),
                page: 1,
                table: "Invented remaining-period limits".into(),
                row: "Example Mine".into(),
                col: quantity.clone(),
            },
            doc_sha256: doc.sha256.clone(),
            token: token.into(),
            value: parse_token(token).map_err(crate::input_refusal)?,
            quantity,
            unit: unit.into(),
            basis: "100%;scenario=declared_upper_bound".into(),
            period: "2030H2".into(),
            readers: vec!["synthetic-reader-a".into(), "synthetic-reader-b".into()],
        });
    }
    Ok(TranscriptionReport {
        registry: "synthetic:invented-technical-two-reader-agreement".into(),
        documents: vec![doc],
        entries: 6,
        agreement: Agreement {
            accepted,
            ..Agreement::default()
        },
        tie: None,
    })
}
