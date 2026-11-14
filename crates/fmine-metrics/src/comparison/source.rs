//! Binding agreed public transcription points to peer comparisons (BD-355).
//! Determinism: Exact
//!
//! The caller authenticates document bytes and complete reader agreement through the existing public
//! transcription loader. Definition fields and their citation must be part of that same agreement.

use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator, Token, parse_token};

use super::*;

fn digest(text: &str) -> Result<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "document needs a lowercase SHA-256",
        ));
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| fail(RefusalCode::NoClaimInDomain201, "invalid document SHA-256"))?;
    }
    if bytes == [0; 32] {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "document SHA-256 is absent",
        ));
    }
    Ok(bytes)
}

/// Bind one source row without unit, ownership, scope, definition or price conversion.
pub fn bind(
    issuer: &str,
    metric: Metric,
    year: u16,
    document: &DocumentRow,
    row: &Accepted,
    evidence: EvidenceClass,
) -> Result<Claim<PeerReading>> {
    if !matches!(document.kind.as_str(), "mda" | "aif" | "partner_filing") {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "actual peer values require a registered annual or financial filing",
        ));
    }
    if row.locator.doc != document.file
        || row.doc_sha256 != document.sha256
        || row.locator.page == 0
        || row.locator.page > document.pages
        || row.period != format!("{year:04}FY")
        || row.unit != metric.unit()
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "source must match selected document, complete annual period and exact metric unit",
        ));
    }
    let readers: BTreeSet<_> = row.readers.iter().collect();
    if readers.len() < 2
        || readers.len() != row.readers.len()
        || readers.iter().any(|r| r.trim().is_empty())
        || parse_token(&row.token).ok() != Some(row.value)
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "value and interpretation require at least two distinct agreeing readers",
        ));
    }
    let Token::Point(point) = row.value else {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "peer actual requires a printed point, not a range or bound",
        ));
    };
    if metric == Metric::GoldProduction && point.mantissa < 0 {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "reported gold production cannot be negative",
        ));
    }
    let denominator = 10_i128.checked_pow(point.scale).ok_or_else(|| {
        fail(
            RefusalCode::NotObservable202,
            "printed decimal scale overflows",
        )
    })?;
    let printed = checked(Rat::new(point.mantissa, denominator))?;
    let half = checked(Rat::new(
        1,
        denominator
            .checked_mul(2)
            .ok_or_else(|| fail(RefusalCode::NotObservable202, "rounding step overflows"))?,
    ))?;
    let interval = Interval {
        lo: checked(printed.checked_sub(half))?,
        hi: checked(printed.checked_add(half))?,
        lo_strict: false,
        hi_strict: false,
    };
    let mut fields = BTreeMap::new();
    for part in row.basis.split(';') {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            fail(
                RefusalCode::NonIdentifiableAttribution104,
                "definition requires explicit key=value fields",
            )
        })?;
        if key.trim() != key
            || value.trim() != value
            || key.is_empty()
            || value.is_empty()
            || fields.insert(key.to_string(), value.to_string()).is_some()
        {
            return Err(fail(
                RefusalCode::NonIdentifiableAttribution104,
                "definition has empty, padded or duplicate fields",
            ));
        }
    }
    let mut citation = |field: &str| {
        fields.remove(field).ok_or_else(|| {
            fail(
                RefusalCode::NonIdentifiableAttribution104,
                format!("missing definition citation {field}"),
            )
        })
    };
    let page = citation("definition_page")?
        .parse::<u64>()
        .ok()
        .filter(|page| *page > 0 && *page <= document.pages)
        .ok_or_else(|| {
            fail(
                RefusalCode::NonIdentifiableAttribution104,
                "definition page is outside the selected document",
            )
        })?;
    let locator = Locator {
        doc: document.file.clone(),
        page,
        table: citation("definition_table")?,
        row: citation("definition_row")?,
        col: citation("definition_col")?,
    };
    let definition = Definition { fields };
    definition.validate(metric)?;
    let sha = digest(&document.sha256)?;
    let path = format!("{}/{}", document.dir, document.file);
    let source = SourceAnchor::new(path.clone(), sha, row.locator.to_string());
    let definition_source = SourceAnchor::new(path, sha, locator.to_string());
    let value = PeerReading {
        issuer: issuer.into(),
        metric,
        year,
        definition,
        interval,
        source: source.clone(),
        definition_source: definition_source.clone(),
        quantity: row.quantity.clone(),
        raw_token: row.token.clone(),
        basis: row.basis.clone(),
        published_at: document.published_at.clone(),
        published_at_basis: document.published_at_basis.clone(),
        readers: readers.into_iter().cloned().collect(),
    };
    let tag = value.canonical_tag();
    let mut anchors = vec![source, definition_source];
    anchors.sort();
    anchors.dedup();
    Ok(Claim::new(
        INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        metric.additivity(),
        evidence,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        anchors,
    ))
}
