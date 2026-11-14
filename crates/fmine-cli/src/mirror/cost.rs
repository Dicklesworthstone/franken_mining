//! DESIGN (BD-346.S5): authenticated cost readings and explicit two-sided dollar bridges.
use super::*;
use fmine_claim::inspection::public_json;
use fmine_metrics::mirror::cost::{
    self as kernel, CostDefinition, CostReading, CostReport, CostValue, DefinitionField,
};

fn definition(value: &CostDefinition) -> String {
    format!(
        "{{{}}}",
        DefinitionField::ALL
            .iter()
            .map(|field| format!(
                "{}:{}",
                j(field.as_str()),
                j(value.fields.get(field).map_or("", String::as_str))
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn value(value: &CostValue) -> String {
    format!(
        "{{\"asset\":{},\"period\":{},\"metric\":{},\"unit\":\"USD_m\",\"perimeter\":{},\"definition\":{},\"interval\":{}}}",
        j(&value.asset),
        j(&period(value.period)),
        j(value.metric.as_str()),
        perimeter(&value.perimeter),
        definition(&value.definition),
        interval(value.interval)
    )
}

fn reading(claim: &Claim<CostReading>) -> Result<String> {
    let source = &claim.value;
    let owner = match source.ownership {
        Ownership::Full => "{\"kind\":\"100%\"}".into(),
        Ownership::Attributable { share } => {
            format!("{{\"kind\":\"attributable\",\"share\":{}}}", rat(share))
        }
    };
    let layer = source.layer.as_ref().map_or("null".into(), |layer| {
        format!(
            "{{\"kind\":{},\"target\":{},\"direction\":\"signed amount added to original cost\"}}",
            j(layer.field.as_str()),
            j(&layer.target)
        )
    });
    public_json(
        claim,
        "jv_cost_source",
        format!(
            "{{\"reporter\":{},\"source\":{},\"definition_source\":{},\"ownership\":{owner},\"cost\":{},\"layer\":{layer},\"basis\":{},\"publication\":{},\"raw_token\":{},\"readers\":[{}]}}",
            j(&source.reporter),
            source_anchor(&source.source),
            source_anchor(&source.definition_source),
            value(&source.value),
            j(&source.basis),
            j(&source.publication),
            j(&source.raw_token),
            source
                .readers
                .iter()
                .map(|reader| j(reader))
                .collect::<Vec<_>>()
                .join(",")
        ),
    )
}

fn source(source: &Source) -> Result<String> {
    let d = &source.document;
    let rows = source.rows.iter().map(|row| format!(
        "{{\"quantity\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"locator\":{},\"readers\":[{}]}}",
        j(&row.quantity), j(&row.token), j(&row.unit), j(&row.basis), j(&row.period), j(&row.locator.to_string()),
        row.readers.iter().map(|v| j(v)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
    let ownership = source
        .ownership
        .as_ref()
        .map(|c| public_json(c, "jv_ownership", rat(c.value)))
        .transpose()?
        .unwrap_or_else(|| "null".into());
    Ok(format!(
        "{{\"issuer\":{},\"file\":{},\"url\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"readings\":[{rows}],\"ownership_claim\":{ownership}}}",
        j(&source.issuer),
        j(&d.file),
        j(&d.url),
        j(&d.sha256),
        j(&d.published_at),
        j(&d.published_at_basis)
    ))
}

fn render(
    selected: &Selection,
    sources: &[Source],
    report: &CostReport,
) -> Result<(String, String)> {
    let inputs = report
        .inputs
        .iter()
        .map(reading)
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let aligned = report
        .aligned
        .iter()
        .map(|(reporter, c)| {
            Ok(format!(
                "{{\"reporter\":{},\"claim\":{}}}",
                j(reporter),
                public_json(c, "jv_cost_aligned_full", value(&c.value))?
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let common = report
        .common
        .as_ref()
        .map(|c| public_json(c, "jv_cost_common_full", value(&c.value)))
        .transpose()?
        .unwrap_or_else(|| "null".into());
    let certificate = report.separation.as_ref().map_or_else(|| "null".into(), |s| format!(
        "{{\"kind\":\"exact_interval_separation\",\"lower_reporter\":{},\"lower\":{},\"upper_reporter\":{},\"upper\":{},\"relation\":\"lower > upper\"}}",
        j(&s.lower_reporter), rat(s.lower), j(&s.upper_reporter), rat(s.upper)));
    let source_json = sources
        .iter()
        .map(source)
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let body = format!(
        "\"schema\":\"fmine.jv_cost_mirror.v1\",\"asset\":{},\"period\":{},\"as_of\":{},\"metric\":{},\"unit\":\"USD_m\",\"synthetic\":false,\"gate_eligible\":false,\"jv_sensitive\":true,\"sources\":[{source_json}],\"inputs\":[{inputs}],\"aligned\":[{aligned}],\"common\":{common},\"layers\":[{}],\"verdict\":{{\"status\":{},\"certificate\":{certificate}}},\"construction\":\"Each printed source plus or minus half its own last place; signed adjustments added before exact ownership division. Independent rounding boxes; no per-ounce rate.\",\"no_claim\":{}",
        j(&selected.asset),
        j(&period(selected.period)),
        j(&selected.as_of),
        j(selected.metric.name()),
        report
            .layers
            .iter()
            .map(|f| j(f.as_str()))
            .collect::<Vec<_>>()
            .join(","),
        j(report.status()),
        j(kernel::NO_CLAIM)
    );
    let digest = blake3_hash(body.as_bytes()).to_hex();
    let json = format!("{{\"payload_digest\":{},{body}}}", j(&digest));
    let mut markdown = format!(
        "# JV cost mirror — {}\n\n{}; as of {}. **{}**. Extensive USD-million costs on the explicitly declared common basis.\n\n{}\n\n| Reporter | Source assertion | Printed value | Definition and bridge | Readers |\n|---|---|---|---|---|\n",
        clean(&selected.asset),
        period(selected.period),
        selected.as_of,
        report.status(),
        kernel::NO_CLAIM
    );
    for source in sources {
        for row in &source.rows {
            let _ = writeln!(
                markdown,
                "| {} | {} | {} {} | {} | {} |",
                clean(&source.issuer),
                clean(&row.locator.to_string()),
                clean(&row.token),
                clean(&row.unit),
                clean(&row.basis),
                clean(&row.readers.join(", "))
            );
        }
        let _ = writeln!(
            markdown,
            "\n{}: SHA-256 `{}`, published {} ({}).\n",
            clean(&source.document.file),
            source.document.sha256,
            source.document.published_at,
            clean(&source.document.published_at_basis)
        );
    }
    markdown.push_str("\n## Amounts on the common definition and 100% basis\n\nEvery adjustment is a separately cited signed amount; no missing adjustment becomes zero. Sold/produced and by-/co-product policy differences are not bridged.\n");
    for (reporter, amount) in &report.aligned {
        let i = amount.value.interval;
        let _ = writeln!(
            markdown,
            "\n{}: [{}/{}, {}/{}] USD million; Claim `{}`.\n",
            clean(reporter),
            i.lo.num(),
            i.lo.den(),
            i.hi.num(),
            i.hi.den(),
            amount.claim_id
        );
    }
    if let Some(common) = &report.common {
        let i = common.value.interval;
        let _ = writeln!(
            markdown,
            "\nCommon compatibility interval: [{}/{}, {}/{}] USD million.\n",
            i.lo.num(),
            i.lo.den(),
            i.hi.num(),
            i.hi.den()
        );
    }
    if let Some(s) = &report.separation {
        let _ = writeln!(
            markdown,
            "\nExact conflict: {} lower bound {}/{} exceeds {} upper bound {}/{}.\n",
            clean(&s.lower_reporter),
            s.lower.num(),
            s.lower.den(),
            clean(&s.upper_reporter),
            s.upper.num(),
            s.upper.den()
        );
    }
    let _ = writeln!(
        markdown,
        "\nPayload digest: `{digest}`. JSON retains original and derived canonical Claim bodies, all source locators, and the exact separation certificate when incompatible."
    );
    Ok((json, markdown))
}

fn run(root: &Path, selected: &Selection, metric: CostMetric) -> Result<(String, String)> {
    if !slug(&selected.asset)
        || parse_period(&period(selected.period)) != Some(selected.period)
        || !is_date(&selected.as_of)
        || selected.reporters[0].issuer == selected.reporters[1].issuer
        || selected
            .reporters
            .iter()
            .any(|r| !is_issuer_name(&r.issuer) || r.issuer == "synthetic")
    {
        return Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "invalid cost asset, period, cut or reporters",
        ));
    }
    let mut sources = Vec::new();
    let mut inputs = Vec::new();
    for reporter in &selected.reporters {
        let (document, rows) = load_rows(root, selected, reporter)?;
        let bound = kernel::bind::bind(
            &selected.asset,
            selected.period,
            metric,
            &reporter.issuer,
            &document,
            &rows,
            EvidenceClass::ISSUER,
        )?;
        inputs.extend(bound.inputs);
        sources.push(Source {
            issuer: reporter.issuer.clone(),
            document,
            rows,
            ownership: bound.ownership,
        });
    }
    sources.sort_by(|a, b| a.issuer.cmp(&b.issuer));
    render(selected, &sources, &kernel::compare(&inputs)?)
}

pub(super) fn execute(root: &Path, selected: &Selection, metric: CostMetric) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "Inspect the cost mirror's explicit source and definition requirements.",
    )];
    match run(root, selected, metric) {
        Ok((json, markdown)) => {
            let mut envelope = Envelope::ok("mirror", json, actions);
            envelope.no_claim = Some(kernel::NO_CLAIM);
            envelope.rendered_markdown = Some(markdown);
            envelope
        }
        Err(refusal) => Envelope::refused("mirror", refusal, actions),
    }
}
