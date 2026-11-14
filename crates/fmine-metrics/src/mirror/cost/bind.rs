//! DESIGN: pure binding of independently agreed cost rows to their own registered document.
//! Source bytes, full reader agreement and publication eligibility are checked by the caller.
//! Determinism: Exact

use super::super::transcriptions as source;
use super::*;

fn base(asset: &str, period: Period, metric: CostMetric) -> String {
    format!(
        "mirror_{}_{}_{}",
        asset.replace('-', "_"),
        source::period_label(period).to_ascii_lowercase(),
        metric.as_str()
    )
}

/// Every supported and forbidden definition-adjustment key is selected, so it cannot disappear at load.
#[must_use]
pub fn keys(asset: &str, period: Period, metric: CostMetric) -> Vec<String> {
    let base = base(asset, period, metric);
    let mut keys = vec![format!("{base}_100_usd_m"), format!("{base}_attr_usd_m")];
    for field in DefinitionField::ALL {
        for mode in ["100", "attr"] {
            keys.push(format!("{base}_{}_adjustment_{mode}_usd_m", field.as_str()));
        }
    }
    keys.push(source::keys(asset, period, MirrorMetric::SoldKoz)[2].clone());
    keys
}

fn definition(
    row: &Accepted,
    document: &DocumentRow,
) -> Result<(
    source::Context,
    CostDefinition,
    SourceAnchor,
    Option<String>,
)> {
    let mut fields = BTreeMap::new();
    let mut parts = row.basis.split(';');
    let mut physical = vec![parts.next().unwrap_or_default()];
    for part in parts {
        let Some((key, value)) = part.split_once('=') else {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "cost basis needs key=value declarations",
            );
        };
        if DefinitionField::ALL.iter().any(|f| f.as_str() == key)
            || matches!(
                key,
                "target"
                    | "definition_page"
                    | "definition_table"
                    | "definition_row"
                    | "definition_col"
            )
        {
            if value.trim().is_empty() || fields.insert(key, value).is_some() {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    "empty or duplicate cost-definition field",
                );
            }
        } else {
            physical.push(part);
        }
    }
    let mut context_row = row.clone();
    context_row.basis = physical.join(";");
    let ctx = source::context(&context_row, document, None)?;
    let get = |key| {
        fields.get(key).copied().ok_or_else(|| {
            Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                format!("{} lacks cost-definition {key}", row.locator),
            )
        })
    };
    let mut values = BTreeMap::new();
    for field in DefinitionField::ALL {
        values.insert(field, get(field.as_str())?.to_string());
    }
    let definition = CostDefinition { fields: values };
    definition.validate()?;
    let page = get("definition_page")?
        .parse::<u64>()
        .ok()
        .filter(|p| *p > 0 && *p <= document.pages)
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                "cost-definition page outside the selected document",
            )
        })?;
    let locator = Locator {
        doc: document.file.clone(),
        page,
        table: get("definition_table")?.into(),
        row: get("definition_row")?.into(),
        col: get("definition_col")?.into(),
    };
    Ok((
        ctx,
        definition,
        source::anchor(document, locator.to_string())?,
        fields.get("target").map(|v| (*v).into()),
    ))
}

/// One original total (full or attributable) per reporter. Signed bridge amounts use that same basis.
/// Different source variants are retained as an ambiguity refusal; values never choose between them.
pub fn bind(
    asset: &str,
    period: Period,
    metric: CostMetric,
    reporter: &str,
    document: &DocumentRow,
    rows: &[Accepted],
    evidence: EvidenceClass,
) -> Result<BoundCosts> {
    let keys = keys(asset, period, metric);
    let lookup = |key: &str| -> Result<Option<&Accepted>> {
        let found = rows
            .iter()
            .filter(|r| r.quantity == key)
            .collect::<Vec<_>>();
        match found.as_slice() {
            [] => Ok(None),
            [row] => Ok(Some(*row)),
            _ => refuse(
                RefusalCode::NotObservable202,
                format!("multiple readings for {key}"),
            ),
        }
    };
    let originals = [lookup(&keys[0])?, lookup(&keys[1])?];
    let (original, attributable) = match originals {
        [Some(row), None] => (row, false),
        [None, Some(row)] => (row, true),
        [None, None] => {
            return refuse(
                RefusalCode::NoClaimInDomain201,
                format!("{reporter} lacks the requested USD-million cost amount"),
            );
        }
        _ => {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                format!("{reporter} has competing full and attributable cost totals"),
            );
        }
    };
    let ownership = if attributable {
        let key = source::keys(asset, period, MirrorMetric::SoldKoz)[2].clone();
        Some(source::share(
            lookup(&key)?.ok_or_else(|| {
                Refusal::new(
                    RefusalCode::NonIdentifiableAttribution104,
                    "cost amount lacks separately cited ownership",
                )
            })?,
            document,
            period,
            evidence,
        )?)
    } else {
        None
    };
    let mut selected = vec![(original, None)];
    for field in DefinitionField::ALL {
        for mode in ["100", "attr"] {
            if let Some(row) = lookup(&format!(
                "{}_{}_adjustment_{mode}_usd_m",
                base(asset, period, metric),
                field.as_str()
            ))? {
                if (mode == "attr") != attributable {
                    return refuse(
                        RefusalCode::BasisMismatch105,
                        "cost adjustment and original use different ownership bases",
                    );
                }
                selected.push((row, Some(field)));
            }
        }
    }
    let mut inputs = Vec::new();
    for (row, field) in selected {
        if row.unit != "USD_m"
            || row.period != source::period_label(period)
            || row.locator.doc != document.file
            || row.doc_sha256 != document.sha256
            || row.locator.page == 0
            || row.locator.page > document.pages
        {
            return refuse(
                RefusalCode::BasisMismatch105,
                "cost row needs selected document, period and explicit USD_m unit",
            );
        }
        let (ctx, definition, definition_source, target) = definition(row, document)?;
        if ctx.mode != if attributable { "attributable" } else { "100%" } {
            return refuse(
                RefusalCode::BasisMismatch105,
                "cost key and printed ownership mode disagree",
            );
        }
        let layer = match (field, target) {
            (None, None) => None,
            (Some(field), Some(target)) => Some(CostLayer { field, target }),
            _ => {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    "each cost adjustment requires one explicit target treatment; originals have none",
                );
            }
        };
        let source_anchor = source::anchor(document, row.locator.to_string())?;
        let mut anchors = vec![source_anchor.clone(), ctx.anchor, definition_source.clone()];
        let mut parents = Vec::new();
        let owner = if let Some((share, share_ctx)) = &ownership {
            if share_ctx.perimeter != ctx.perimeter {
                return refuse(
                    RefusalCode::SupportMismatch102,
                    "cost amount and ownership have different dated perimeters",
                );
            }
            anchors.extend(share.anchors.clone());
            parents.push(share.claim_id);
            Ownership::Attributable { share: share.value }
        } else {
            Ownership::Full
        };
        let (printed, step) = source::point(row)?;
        let half = checked(step.checked_div(Rat::from_integer(2)))?;
        let value = CostReading {
            reporter: reporter.into(),
            source: source_anchor,
            definition_source,
            ownership: owner,
            layer,
            basis: row.basis.clone(),
            publication: format!("{};{}", document.published_at, document.published_at_basis),
            raw_token: row.token.clone(),
            readers: row.readers.clone(),
            value: CostValue {
                asset: asset.into(),
                period,
                metric,
                perimeter: ctx.perimeter,
                definition,
                interval: xf::Interval {
                    lo: checked(printed.checked_sub(half))?,
                    hi: checked(printed.checked_add(half))?,
                    lo_strict: false,
                    hi_strict: false,
                },
            },
        };
        let tag = value.canonical_tag();
        let claim = source::new_claim(
            SOURCE_METRIC,
            value,
            &tag,
            evidence,
            AdditivityClass::ExtensiveAdditive,
            parents,
            anchors,
        );
        validate(&claim)?;
        inputs.push(claim);
    }
    Ok(BoundCosts {
        ownership: ownership.map(|s| s.0),
        inputs,
    })
}
