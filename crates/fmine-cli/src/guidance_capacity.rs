//! Optional registered technical-report upper bounds for guidance completion (BD-342).
//! DESIGN: only explicitly asserted upper limits over the exact remaining months enter this adapter.
//! Annual rates, averages, nominal nameplates and percentages are never converted into physical ceilings.
//! Determinism: DeterministicPerIsa (exact canonical values; approximate Markdown displays).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Digest32, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_metrics::capacity::{
    CapacityLimits, CapacityReport, CapacityVerdict, PhysicalKind, PhysicalLimit,
    compare_production,
};
use fmine_metrics::guidance::{Ownership, Period, Quantity};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::guidance::GuidanceCompletionReport;
use fmine_public::sources::{DocumentRow, load_issuer_profile};
use fmine_public::transcribe::load_transcription_report_as_of;
use fmine_registry::sources::{is_date, is_quarter};
use fmine_registry::transcriptions::{Accepted, Token};

const BASIS: &str = "100%;bound=explicit-upper;metal=gold";
pub(crate) const NO_CLAIM: &str = "Conditional gold-production upper bound from explicitly stated limits; no physical feasibility, joint production-cost witness, probability, mine-impossibility or regulatory claim.";
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone)]
pub(crate) struct GuidanceCapacityReport {
    pub limits: CapacityLimits,
    pub comparison: CapacityReport,
    pub document: DocumentRow,
    pub sources: Vec<Accepted>,
    pub guidance_digest: Digest32,
    pub digest: Digest32,
    pub issuer: String,
    pub site: String,
    pub through: String,
    pub as_of: String,
}

fn refuse(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance capacity: {}", reason.into()))
}

fn key(subject: &str) -> String {
    subject
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn period(p: Period) -> String {
    format!(
        "{:04}-{:02}/{:04}-{:02}",
        p.year, p.first_month, p.year, p.last_month
    )
}

fn sha_bytes(hex: &str) -> Result<[u8; 32]> {
    let mut out = [0; 32];
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            "malformed document digest",
        ));
    }
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|_| refuse(RefusalCode::DigestMismatch505, "malformed document digest"))?;
    }
    Ok(out)
}

fn document_admitted(document: &DocumentRow, as_of: &str) -> Result<()> {
    if !is_date(as_of) {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "as-of needs YYYY-MM-DD",
        ));
    }
    if document.kind != "technical_report" {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "selected document must be registered as technical_report",
        ));
    }
    if document.published_at.as_str() > as_of {
        return Err(refuse(
            RefusalCode::AssayLookahead108,
            format!(
                "{} was published after the requested as-of date",
                document.file
            ),
        ));
    }
    Ok(())
}

fn limit(
    row: &Accepted,
    document: &DocumentRow,
    required: &Claim<Quantity>,
    kind: PhysicalKind,
    unit: &str,
    evidence: EvidenceClass,
) -> Result<Claim<PhysicalLimit>> {
    if row.locator.doc != document.file || row.doc_sha256 != document.sha256 {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            "limit names different document bytes",
        ));
    }
    if row.readers.iter().collect::<BTreeSet<_>>().len() < 2 {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "limit needs two agreeing reader roles",
        ));
    }
    if row.unit != unit {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!(
                "{} needs unit {unit}; no conversion is inferred",
                row.locator
            ),
        ));
    }
    let labels = format!(
        "{} {} {}",
        row.locator.table, row.locator.row, row.locator.col
    )
    .to_ascii_lowercase();
    let explicit_label = ["maximum", "upper limit", "at most"]
        .iter()
        .any(|s| labels.contains(s));
    if row.basis != BASIS || !explicit_label {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "{} needs the explicit-upper basis and a printed maximum, upper limit or at most label; an estimate is not a limit",
                row.locator
            ),
        ));
    }
    if row.period != period(required.value.period) {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            format!(
                "{} needs exact remaining-month support {}",
                row.locator,
                period(required.value.period)
            ),
        ));
    }
    let Token::Point(value) = row.value else {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "a physical upper limit must be one stated number, not a guidance range",
        ));
    };
    let denominator = 10_i128.checked_pow(value.scale).ok_or_else(|| {
        refuse(
            RefusalCode::NumericalCertificateFailed207,
            "limit decimal scale overflow",
        )
    })?;
    let upper = Rat::new(value.mantissa, denominator).map_err(|_| {
        refuse(
            RefusalCode::NumericalCertificateFailed207,
            "limit decimal overflow",
        )
    })?;
    let value = PhysicalLimit {
        subject: required.value.subject.clone(),
        period: required.value.period,
        ownership: Ownership::Full,
        kind,
        interval: Interval {
            lo: Rat::from_integer(0),
            hi: upper,
            lo_strict: false,
            hi_strict: false,
        },
    };
    let tag = value.canonical_tag();
    Ok(Claim::new(
        &row.quantity,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        if kind == PhysicalKind::RemainingDryFeedMt {
            AdditivityClass::ExtensiveAdditive
        } else {
            AdditivityClass::IntensiveWeighted
        },
        evidence,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            format!("{}/{}", document.dir, document.file),
            sha_bytes(&document.sha256)?,
            row.locator.to_string(),
        )],
    ))
}

fn limits_from_rows(
    rows: &[Accepted],
    document: &DocumentRow,
    required: &Claim<Quantity>,
    evidence: EvidenceClass,
) -> Result<(CapacityLimits, Vec<Accepted>)> {
    let site = key(&required.value.subject).replace('-', "_");
    let specs = [
        (
            "remaining_dry_feed_mt",
            PhysicalKind::RemainingDryFeedMt,
            "dry_mt",
        ),
        (
            "head_grade_g_per_t",
            PhysicalKind::HeadGradeGPerT,
            "g_per_dry_t",
        ),
        (
            "recovery_fraction",
            PhysicalKind::RecoveryFraction,
            "fraction",
        ),
    ];
    let mut values = Vec::new();
    let mut sources = Vec::new();
    for (suffix, kind, unit) in specs {
        let quantity = format!("capacity_{site}_{suffix}");
        let matching: Vec<_> = rows
            .iter()
            .filter(|r| r.locator.doc == document.file && r.quantity == quantity)
            .collect();
        let [row] = matching.as_slice() else {
            return Err(refuse(
                RefusalCode::NotObservable202,
                format!(
                    "{} needs exactly one two-reader {quantity} row",
                    document.file
                ),
            ));
        };
        values.push(limit(row, document, required, kind, unit, evidence)?);
        sources.push((*row).clone());
    }
    let mut values = values.into_iter();
    let missing = || {
        refuse(
            RefusalCode::NotObservable202,
            "incomplete physical-limit set",
        )
    };
    let limits = CapacityLimits {
        feed: values.next().ok_or_else(missing)?,
        grade: values.next().ok_or_else(missing)?,
        recovery: values.next().ok_or_else(missing)?,
    };
    Ok((limits, sources))
}

/// Called only for an explicitly selected capacity document. Baseline guidance completion stays independent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assess(
    root: &Path,
    issuer: &str,
    site: &str,
    through: &str,
    as_of: &str,
    document: &str,
    completion: &GuidanceCompletionReport,
) -> Result<GuidanceCapacityReport> {
    let required = &completion.completion.remaining_production;
    required.check_id()?;
    if required.value_tag() != required.value.canonical_tag() {
        return Err(refuse(
            RefusalCode::FrozenRecordMutation507,
            "remaining-production value does not match its canonical Claim",
        ));
    }
    if key(&required.value.subject) != site || !is_quarter(through) {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "site or release does not match the guidance report",
        ));
    }
    let year = through[..4]
        .parse::<u16>()
        .map_err(|_| refuse(RefusalCode::SupportMismatch102, "invalid release year"))?;
    let quarter = through.as_bytes()[5] - b'0';
    if required.value.period
        != (Period {
            year,
            first_month: quarter * 3 + 1,
            last_month: 12,
        })
    {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "release and remaining months differ",
        ));
    }
    let profile = load_issuer_profile(root, issuer)?.through(through)?;
    let document = profile
        .documents
        .iter()
        .find(|d| d.file == document)
        .ok_or_else(|| {
            refuse(
                RefusalCode::NoClaimInDomain201,
                "capacity document is not registered",
            )
        })?
        .clone();
    document_admitted(&document, as_of)?;
    let transcript = load_transcription_report_as_of(&profile, root, as_of)?.ok_or_else(|| {
        refuse(
            RefusalCode::NotObservable202,
            "two-reader limit transcriptions are absent",
        )
    })?;
    if !transcript
        .documents
        .iter()
        .any(|d| d.file == document.file && d.sha256 == document.sha256)
    {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "selected document has no checked transcription rows",
        ));
    }
    let source_evidence = if issuer == fmine_public::synthetic::SYNTHETIC_ISSUER {
        EvidenceClass::Assumed
    } else {
        EvidenceClass::ISSUER
    };
    let (limits, sources) = limits_from_rows(
        &transcript.agreement.accepted,
        &document,
        required,
        source_evidence,
    )?;
    let comparison = compare_production(required, &limits)?;
    let mut report = GuidanceCapacityReport {
        limits,
        comparison,
        document,
        sources,
        guidance_digest: completion.digest,
        digest: Digest32([0; 32]),
        issuer: issuer.into(),
        site: site.into(),
        through: through.into(),
        as_of: as_of.into(),
    };
    report.digest = blake3_hash(report.body().as_bytes());
    Ok(report)
}

fn j(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn rational(r: Rat) -> String {
    format!("{{\"num\":\"{}\",\"den\":\"{}\"}}", r.num(), r.den())
}

fn interval(i: Interval) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
        rational(i.lo),
        rational(i.hi),
        i.lo_strict,
        i.hi_strict
    )
}

fn provenance<T>(c: &Claim<T>) -> String {
    format!(
        "\"claim_id\":{},\"evidence\":{},\"parents\":[{}],\"anchors\":[{}]",
        j(&c.claim_id.to_string()),
        j(&format!("{:?}", c.evidence)),
        c.parents
            .iter()
            .map(|p| j(&p.to_string()))
            .collect::<Vec<_>>()
            .join(","),
        c.anchors
            .iter()
            .map(|a| format!(
                "{{\"artifact\":{},\"sha256\":{},\"locator\":{}}}",
                j(&a.artifact_path),
                j(&Digest32(a.artifact_sha256).to_hex()),
                j(&a.locator),
            ))
            .collect::<Vec<_>>()
            .join(","),
    )
}

impl GuidanceCapacityReport {
    fn verdict(&self) -> &'static str {
        match self.comparison.verdict {
            CapacityVerdict::ExceedsStatedProductionCeiling => "exceeds_stated_production_ceiling",
            CapacityVerdict::NotExcluded => "not_excluded",
            CapacityVerdict::AlreadySatisfied => "already_satisfied",
        }
    }

    fn body(&self) -> String {
        let sources = self.sources.iter().zip([
            &self.limits.feed, &self.limits.grade, &self.limits.recovery,
        ]).map(|(row, claim)| format!(
            "{{\"quantity\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"locator\":{},\
             \"sha256\":{},\"readers\":[{}],\"bounds\":{},{}}}",
            j(&row.quantity), j(&row.token), j(&row.unit), j(&row.basis), j(&row.period),
            j(&row.locator.to_string()), j(&row.doc_sha256),
            row.readers.iter().map(|r| j(r)).collect::<Vec<_>>().join(","),
            interval(claim.value.interval), provenance(claim),
        )).collect::<Vec<_>>().join(",");
        let maximum = &self.comparison.maximum_production;
        let grade = &self.comparison.optimistic_required_grade;
        let ownership = match maximum.value.ownership {
            Ownership::Full => "\"100%\"".to_string(),
            Ownership::Attributable { share } => {
                format!("{{\"attributable_share\":{}}}", rational(share))
            }
        };
        format!(
            "\"schema\":\"fmine.guidance.capacity.v1\",\"tier\":\"T2\",\"scope\":\"gold_production_ceiling_only\",\
             \"issuer\":{},\"site\":{},\"through\":{},\"as_of\":{},\"guidance_digest\":{},\
             \"document\":{{\"file\":{},\"url\":{},\"published_at\":{},\"published_at_basis\":{},\"sha256\":{}}},\
             \"limits\":[{}],\"verdict\":{},\"maximum_production\":{{\"unit\":\"koz\",\"ownership\":{},\
             \"period\":{},\"value\":{},\"rule\":{},{}}},\"optimistic_required_grade\":{{\"unit\":\"g_per_dry_t\",\
             \"ownership\":\"100%\",\"period\":{},\"value\":{},\"rule\":{},{}}},\"no_claim\":{}",
            j(&self.issuer),
            j(&self.site),
            j(&self.through),
            j(&self.as_of),
            j(&self.guidance_digest.to_hex()),
            j(&self.document.file),
            j(&self.document.url),
            j(&self.document.published_at),
            j(&self.document.published_at_basis),
            j(&self.document.sha256),
            sources,
            j(self.verdict()),
            ownership,
            j(&period(maximum.value.period)),
            interval(maximum.value.interval),
            j(&maximum.metric_key),
            provenance(maximum),
            j(&period(grade.value.period)),
            interval(grade.value.interval),
            j(&grade.metric_key),
            provenance(grade),
            j(NO_CLAIM),
        )
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"digest\":{},{}}}",
            j(&self.digest.to_hex()),
            self.body()
        )
    }

    pub(crate) fn to_markdown(&self) -> String {
        use crate::guidance::{interval as display, text};
        let mut out = format!(
            "## T2: explicit technical-report production limits\n\n{}\n\n\
             Remaining support: {}. Selected document: {} (published {}).\n\n\
             **Result:** {}. Maximum production: {} koz, on the guidance ownership basis. \
             Required full-mine head grade at maximum feed and recovery: {} g per dry metric tonne.\n\n\
             These upper limits do not establish that the maxima are simultaneously attainable. \
             The T1 historical joint production-cost comparison remains separate.\n\n\
             | Explicit upper limit | Stated value | Unit | Page and table locator | SHA-256 |\n\
             |---|---|---|---|---|\n",
            NO_CLAIM,
            period(self.limits.feed.value.period),
            text(&self.document.file),
            self.document.published_at,
            self.verdict(),
            display(self.comparison.maximum_production.value.interval),
            display(self.comparison.optimistic_required_grade.value.interval),
        );
        for row in &self.sources {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                text(&row.quantity),
                text(&row.token),
                text(&row.unit),
                text(&row.locator.to_string()),
                row.doc_sha256
            );
        }
        let _ = writeln!(
            out,
            "\nCapacity report BLAKE3: {}. Source guidance report: {}.\n",
            self.digest.to_hex(),
            self.guidance_digest.to_hex()
        );
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use fmine_core::sha256;
    use fmine_registry::transcriptions::{Locator, parse_token};

    const BYTES: &[u8] = b"SYNTHETIC technical-report upper bounds; not issuer data.";

    fn document() -> DocumentRow {
        DocumentRow {
            file: "Bounds.pdf".into(),
            dir: "corpus".into(),
            kind: "technical_report".into(),
            url: "https://example.invalid/Bounds.pdf".into(),
            url_basis: "W1 table".into(),
            published_at: "2030-07-01".into(),
            published_at_basis: "FACT: synthetic publication date".into(),
            pages: 1,
            bytes: BYTES.len() as u64,
            sha256: sha256(BYTES).to_hex(),
        }
    }

    fn rows() -> Vec<Accepted> {
        [
            ("remaining_dry_feed_mt", "10", "dry_mt"),
            ("head_grade_g_per_t", "2", "g_per_dry_t"),
            ("recovery_fraction", "0.90", "fraction"),
        ]
        .into_iter()
        .map(|(suffix, token, unit)| Accepted {
            locator: Locator {
                doc: "Bounds.pdf".into(),
                page: 1,
                table: "Explicit maximum limits".into(),
                row: suffix.into(),
                col: "Remaining months".into(),
            },
            doc_sha256: sha256(BYTES).to_hex(),
            token: token.into(),
            value: parse_token(token).unwrap(),
            quantity: format!("capacity_example_mine_{suffix}"),
            unit: unit.into(),
            basis: BASIS.into(),
            period: "2030-07/2030-12".into(),
            readers: vec!["synthetic-a".into(), "synthetic-b".into()],
        })
        .collect()
    }

    fn required() -> Claim<Quantity> {
        let base = fmine_public::guidance::run_synthetic().unwrap();
        let mut value = base.completion.remaining_production.value.clone();
        value.ownership = Ownership::Full;
        value.interval = Interval {
            lo: Rat::from_integer(600),
            hi: Rat::from_integer(600),
            lo_strict: false,
            hi_strict: false,
        };
        let tag = value.canonical_tag();
        Claim::new(
            "synthetic.remaining-production",
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            base.completion.remaining_production.anchors.clone(),
        )
    }

    #[test]
    fn explicit_caps_reproduce_cod01_exactly() {
        let required = required();
        let (limits, _) =
            limits_from_rows(&rows(), &document(), &required, EvidenceClass::Assumed).unwrap();
        let result = compare_production(&required, &limits).unwrap();
        let grams = Rat::new(311_034_768, 10_000_000).unwrap();
        let expected = Rat::from_integer(18_000).checked_div(grams).unwrap();
        assert_eq!(
            result.verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
        assert_eq!(result.maximum_production.value.interval.lo, expected);
        assert_eq!(result.maximum_production.value.interval.hi, expected);
        let needed = Rat::from_integer(600)
            .checked_mul(grams)
            .unwrap()
            .checked_div(Rat::from_integer(9_000))
            .unwrap();
        assert_eq!(result.optimistic_required_grade.value.interval.lo, needed);
        assert_eq!(result.maximum_production.evidence, EvidenceClass::Assumed);
        assert!(
            result
                .maximum_production
                .anchors
                .iter()
                .any(|a| a.locator.contains("Bounds.pdf"))
        );
    }

    #[test]
    fn limits_refuse_estimates_units_support_ranges_and_missing_agreement() {
        let required = required();
        for case in 0..7 {
            let mut inputs = rows();
            let code = match case {
                0 => {
                    inputs[0].unit = "wet_mt".into();
                    RefusalCode::BasisMismatch105
                }
                1 => {
                    inputs[0].basis = "100%".into();
                    RefusalCode::UndeclaredUnitBasisOrTz107
                }
                2 => {
                    inputs[0].period = "2030FY".into();
                    RefusalCode::SupportMismatch102
                }
                3 => {
                    inputs[0].value = parse_token("8 - 10").unwrap();
                    RefusalCode::UndeclaredUnitBasisOrTz107
                }
                4 => {
                    inputs[0].readers.pop();
                    RefusalCode::NotObservable202
                }
                5 => {
                    inputs[0].locator.table = "Ordinary annual estimates".into();
                    RefusalCode::UndeclaredUnitBasisOrTz107
                }
                _ => {
                    inputs.pop();
                    RefusalCode::NotObservable202
                }
            };
            assert_eq!(
                limits_from_rows(&inputs, &document(), &required, EvidenceClass::Assumed)
                    .unwrap_err()
                    .code,
                code.spec().code
            );
        }
        let mut future = document();
        future.published_at = "2030-08-01".into();
        assert_eq!(
            document_admitted(&future, "2030-07-31").unwrap_err().code,
            RefusalCode::AssayLookahead108.spec().code
        );
        future = document();
        future.kind = "investor_deck".into();
        assert_eq!(
            document_admitted(&future, "2030-07-31").unwrap_err().code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );
    }

    #[test]
    fn registered_document_binding_and_byte_tampering() {
        let root = std::env::temp_dir().join(format!("fmine-capacity-e2e-{}", std::process::id()));
        fmine_public::synthetic::write_issuer_workspace(&root).unwrap();
        std::fs::write(root.join("corpus/Bounds.pdf"), BYTES).unwrap();
        let registry = root.join("registries/sources/synthetic.toml");
        let mut text = std::fs::read_to_string(&registry).unwrap();
        let doc = document();
        text.push_str(&format!(
            "\n[[document]]\nfile = \"Bounds.pdf\"\nkind = \"technical_report\"\n\
             url = \"https://example.invalid/Bounds.pdf\"\nurl_basis = \"W1 table\"\n\
             published_at = \"2030-07-01\"\npublished_at_basis = \"FACT: synthetic date\"\n\
             pages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            BYTES.len(),
            doc.sha256,
        ));
        std::fs::write(&registry, text).unwrap();
        let mut transcript =
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
        for row in rows() {
            for reader in &row.readers {
                let _ = write!(
                    transcript,
                    "\n[[entry]]\ndoc = {}\ndoc_sha256 = {}\npage = 1\ntable = {}\nrow = {}\ncol = {}\n\
                     token = {}\nquantity = {}\nunit = {}\nbasis = {}\nperiod = {}\nreader = {}\n",
                    j(&row.locator.doc),
                    j(&row.doc_sha256),
                    j(&row.locator.table),
                    j(&row.locator.row),
                    j(&row.locator.col),
                    j(&row.token),
                    j(&row.quantity),
                    j(&row.unit),
                    j(&row.basis),
                    j(&row.period),
                    j(reader),
                );
            }
        }
        std::fs::create_dir_all(root.join("registries/transcriptions")).unwrap();
        std::fs::write(
            root.join("registries/transcriptions/synthetic.toml"),
            transcript,
        )
        .unwrap();
        let base = fmine_public::guidance::run_synthetic().unwrap();
        let report = assess(
            &root,
            "synthetic",
            "example-mine",
            "2030Q2",
            "2030-07-31",
            "Bounds.pdf",
            &base,
        )
        .unwrap();
        assert_eq!(report.sources.len(), 3);
        for claim in [
            &report.limits.feed,
            &report.limits.grade,
            &report.limits.recovery,
        ] {
            assert_eq!(claim.evidence, EvidenceClass::Assumed);
            claim.check_id().unwrap();
        }
        assert_eq!(report.comparison.verdict, CapacityVerdict::NotExcluded);
        assert_eq!(report.guidance_digest, base.digest);
        assert!(
            report
                .to_json()
                .contains("\"scope\":\"gold_production_ceiling_only\"")
        );
        assert!(
            report
                .to_markdown()
                .contains("T1 historical joint production-cost comparison remains separate")
        );
        assert_eq!(
            report.digest,
            assess(
                &root,
                "synthetic",
                "example-mine",
                "2030Q2",
                "2030-07-31",
                "Bounds.pdf",
                &base
            )
            .unwrap()
            .digest
        );
        let envelope =
            crate::guidance_capacity_envelope(&base, &report, "Invented synthetic evidence.");
        assert_eq!(envelope.status, "ok");
        assert!(envelope.anchor_id.is_none() && envelope.certificate.is_none());
        assert!(
            envelope
                .payload_json
                .contains("\"schema\":\"fmine.guidance.capacity-completion.v1\"")
        );
        assert!(envelope.payload_json.contains(&base.digest.to_hex()));
        assert!(envelope.payload_json.contains(&report.digest.to_hex()));
        let markdown = envelope.rendered_markdown.as_deref().unwrap();
        assert!(markdown.contains("T2: explicit technical-report production limits"));
        assert!(markdown.contains("Historical joint comparison"));
        assert!(markdown.contains("Composite report BLAKE3"));
        assert!(!markdown.contains("no probability of meeting guidance or technical-report capacity claim is made"));
        assert_eq!(
            envelope.to_json(),
            crate::guidance_capacity_envelope(&base, &report, "Invented synthetic evidence.").to_json()
        );
        std::fs::write(root.join("corpus/Bounds.pdf"), b"SYNTHETIC changed bytes").unwrap();
        assert_eq!(
            assess(
                &root,
                "synthetic",
                "example-mine",
                "2030Q2",
                "2030-07-31",
                "Bounds.pdf",
                &base
            )
            .unwrap_err()
            .code,
            RefusalCode::DigestMismatch505.spec().code
        );
    }
}

