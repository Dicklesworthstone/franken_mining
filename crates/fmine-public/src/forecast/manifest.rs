//! Bind a frozen forecast's information set to its issuer registry.
use fmine_calib::record::{self, InputDoc, Quarter, RecordManifest, RecordRules};
use fmine_evidence::{Refusal, RefusalCode};

use crate::sources::IssuerProfile;

/// Registry location and evidence for checking the frozen source bytes.
#[derive(Debug)]
pub struct RegisteredInput<'a> {
    pub dir: &'a str,
    pub published_at: Option<&'a str>,
    pub published_at_basis: Option<&'a str>,
    url: &'a str,
    bytes: u64,
    digest: &'a str,
    workbook_quarter: Option<&'a str>,
}

fn lookup<'a>(profile: &'a IssuerProfile, file: &str) -> Option<RegisteredInput<'a>> {
    if let Some(s) = profile.sources.iter().find(|s| s.file_name == file) {
        return Some(RegisteredInput {
            dir: &profile.corpus_dir,
            url: &s.url,
            bytes: s.bytes,
            digest: &s.sha256_hex,
            published_at: s.published_at.as_deref(),
            published_at_basis: s.published_at_basis.as_deref(),
            workbook_quarter: Some(&s.quarter),
        });
    }
    if let Some(d) = profile.documents.iter().find(|d| d.file == file) {
        return Some(RegisteredInput {
            dir: &d.dir,
            url: &d.url,
            bytes: d.bytes,
            digest: &d.sha256,
            published_at: Some(&d.published_at),
            published_at_basis: Some(&d.published_at_basis),
            workbook_quarter: None,
        });
    }
    profile.html_sources.iter().find(|d| d.file == file).map(|d| RegisteredInput {
        dir: &d.dir,
        url: &d.url,
        bytes: d.bytes,
        digest: &d.sha256,
        published_at: Some(&d.published_at),
        published_at_basis: Some(&d.published_at_basis),
        workbook_quarter: None,
    })
}

fn check_input<'a>(
    profile: &'a IssuerProfile,
    input: &InputDoc,
) -> Result<Option<RegisteredInput<'a>>, Refusal> {
    let error = |code, why| Refusal::new(code, format!("{}: {why}", input.doc_id));
    let timing = RefusalCode::AssayLookahead108;
    if input.retrieved_utc.div_euclid(86_400) < input.publication_day {
        return Err(error(timing, "declared retrieval precedes publication"));
    }
    let Some(source) = lookup(profile, &input.doc_id) else {
        return Ok(None);
    };
    if record::parse_hex32(source.digest) != Some(input.sha256)
        || input.url != source.url
        || input.bytes != source.bytes
    {
        return Err(error(
            RefusalCode::DigestMismatch505,
            "URL, size or sha256 differs from the source registry",
        ));
    }
    if let Some(quarter) = source.workbook_quarter {
        let end = Quarter::parse(quarter)
            .and_then(|q| q.end_utc())
            .ok_or_else(|| error(RefusalCode::UndeclaredUnitBasisOrTz107, "invalid registered release quarter"))?;
        if input.publication_day < end.div_euclid(86_400) {
            return Err(error(timing, "declared publication precedes completion of the registered quarter"));
        }
    }
    if source.published_at.is_some_and(|day| record::parse_date(day) != Some(input.publication_day)) {
        return Err(error(RefusalCode::DigestMismatch505, "publication day differs from the source registry"));
    }
    Ok(Some(source))
}

/// Verify declarations without opening source bytes. Each result matches its input in order.
/// Separately declared inputs have no registry location. Byte-verifying preflight requires one.
pub fn bind<'a>(
    profile: &'a IssuerProfile,
    rules: &RecordRules,
    manifest: &RecordManifest,
) -> Result<Vec<Option<RegisteredInput<'a>>>, Refusal> {
    if profile.issuer != rules.issuer {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "source registry issuer differs from the frozen rules",
        ));
    }
    let findings = record::validate(manifest, &[], None, rules.dates);
    if !findings.is_empty() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            findings.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        ));
    }
    manifest.inputs.iter().map(|input| check_input(profile, input)).collect()
}
