//! DESIGN (BD-166, BD-188): source-bound, spatially located assay composites.
//! Determinism: DeterministicPerIsa (binary64 grade bounds and desurvey covariance).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation};
use fmine_core::anchor::SourceAnchor;
use fmine_drill::composite::{Bounds, Composite, Domain, Residual};
use fmine_drill::join::Span;
use fmine_drill::survey_error::{self, ErrorModel, UncertainTrace};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal};
use fmine_io::acquire::{
    AcquireExportBundle,
    composite::{
        CompositeSpec, HoleDomains, PreparedComposites, SourceObservation, prepare_composites,
    },
    drill::PreparedHole,
};
use fmine_io::dialects::{ColumnBasis, ColumnSupport, ResolvedUnit};
use fmine_registry::toml_subset::TomlDoc;
use fmine_units::ExactQty;

use super::manifest::{Manifest, fields, get, invalid};
use super::render::{exact, optional, position, quoted, reading};
use crate::worksheet_input::markdown_cell;

pub(super) const SCHEMA: &str = "fmine.drill-composite.v1";
pub const VERB: &str = "drill composite";
pub const USAGE: &str = "drill composite --input <manifest.toml> --cas <directory>";
pub(super) const NO_CLAIM: &str = "Composites are length-weighted over sampled support only, in the declared assay unit, basis and method. Censored readings remain bounds; missing core is not imputed. Coordinates and first-order covariance are conditional on supplied geometry and error declarations. Source methods absent from exports remain explicit assumptions. Original export bytes are retained in the selected CAS. No assay QA/QC pass, density or mass weighting, grade uncertainty distribution, resource estimate, custody chain, authority-ledger commit, reserve disclosure, execution authorization, geotechnical sign-off or regulatory compliance is established.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.drill-composite.v1","#,
    r#""command":"drill composite --input <manifest.toml> --cas <directory>","#,
    r#""input":"Same bounded quoted TOML, tables, holes and error declarations as drill desurvey, with the composite schema and additional tables below","#,
    r#""composite_fields":["column","method","unit","basis","length_m","residual","minimum_m"],"#,
    r#""policy":"Exactly one [[composite]]; explicit matching registered unit, basis and PointCore source support; all present source methods must match method; absent source methods are marked assumed","#,
    r#""residual":"keep, merge_below, drop_below, or distribute; minimum_m required only for merge_below and drop_below","#,
    r#""domain_fields":["hole_id","code","from_m","to_m"],"#,
    r#""domains":"[[domain]] runs must tile every hole from zero through MAXDEPTH, without gaps or overlaps; no output crosses a domain contact","#,
    r#""depths":"Decimal metres must convert exactly to integer millimetres; no rounding","#,
    r#""output":"Per-composite typed Claim with actual support length, grade bounds, coverage, source contribution lengths, start/end coordinates and covariance; original exact readings and missing observations; kept and dropped residuals; conservation including dropped pieces","#,
    r#""limits":"4096 holes; 20000 total composite pieces including dropped residuals; original geometry input-position limit also applies","#,
    r#""example":"tests/drill/composite_synthetic.toml","#,
    r#""side_effect":"Only original export bytes are written to the explicitly selected CAS; no authority records are written"}"#,
);

pub(super) struct Declaration {
    pub spec: CompositeSpec,
    pub domains: Vec<HoleDomains>,
}

fn millimetres(value: &str) -> Result<u64, Refusal> {
    let quantity = ExactQty::from_decimal_str(value)
        .and_then(|q| q.checked_mul(ExactQty::new(1000, 1)?))
        .map_err(|e| invalid(format!("invalid composite depth {value}: {e}")))?;
    if quantity.den() != 1 || quantity.num() < 0 || quantity.num() > (1_i128 << 53) {
        return Err(invalid(
            "composite depths must be nonnegative exact integer millimetres, at most 2^53",
        ));
    }
    Ok(quantity.num() as u64)
}

pub(super) fn parse(doc: &TomlDoc) -> Result<Declaration, Refusal> {
    let policies = doc.rows_of("composite").collect::<Vec<_>>();
    let [policy] = policies.as_slice() else {
        return Err(invalid("exactly one [[composite]] policy is required"));
    };
    let f = fields(
        &policy.fields,
        &["column", "method", "unit", "basis", "length_m", "residual"],
        &["minimum_m"],
    )?;
    let residual = get(&f, "residual")?;
    if matches!(residual, "merge_below" | "drop_below") != f.contains_key("minimum_m") {
        return Err(invalid(
            "minimum_m is required only for merge_below and drop_below",
        ));
    }
    let residual = match residual {
        "keep" => Residual::Keep,
        "distribute" => Residual::Distribute,
        "merge_below" => Residual::MergeBelow {
            min_mm: millimetres(get(&f, "minimum_m")?)?,
        },
        "drop_below" => Residual::DropBelow {
            min_mm: millimetres(get(&f, "minimum_m")?)?,
        },
        _ => return Err(invalid("unknown residual policy")),
    };
    let spec = CompositeSpec {
        column: get(&f, "column")?.into(),
        method: get(&f, "method")?.into(),
        unit: get(&f, "unit")?.into(),
        basis: ColumnBasis::parse(get(&f, "basis")?)
            .ok_or_else(|| invalid("unknown composite basis"))?,
        length_mm: millimetres(get(&f, "length_m")?)?,
        residual,
    };
    let mut domains: BTreeMap<String, Vec<Domain>> = BTreeMap::new();
    for row in doc.rows_of("domain") {
        let f = fields(&row.fields, &["hole_id", "code", "from_m", "to_m"], &[])?;
        domains
            .entry(get(&f, "hole_id")?.into())
            .or_default()
            .push(Domain {
                code: get(&f, "code")?.into(),
                span: Span {
                    from_mm: millimetres(get(&f, "from_m")?)?,
                    to_mm: millimetres(get(&f, "to_m")?)?,
                },
            });
    }
    Ok(Declaration {
        spec,
        domains: domains
            .into_iter()
            .map(|(hole_id, domains)| HoleDomains { hole_id, domains })
            .collect(),
    })
}

fn bounds(value: Option<Bounds>) -> String {
    value.map_or_else(
        || "null".into(),
        |b| format!("{{\"lower\":{},\"upper\":{}}}", b.lo, b.hi),
    )
}

fn source(observation: &SourceObservation, spec: &CompositeSpec) -> String {
    let assay = &observation.source;
    let raw = &assay.source;
    format!(
        "{{\"source_row\":{},\"sample_id\":{},\"from_mm\":{},\"to_mm\":{},\"from_m_exact\":{},\"to_m_exact\":{},\"lab_job\":{},\"source_method\":{},\"method_assumed\":{},\"reading\":{}}}",
        raw.row_index,
        optional(raw.sample_id.as_deref()),
        assay.from_mm,
        assay.to_mm,
        exact(raw.from_m),
        exact(raw.to_m),
        optional(raw.lab_job.as_deref()),
        optional(raw.method.as_deref()),
        observation.method_assumed,
        raw.assays
            .get(&spec.column)
            .map_or_else(|| "null".into(), reading),
    )
}

fn piece(
    value: &Composite,
    hole: &PreparedComposites,
    trace: &UncertainTrace,
    spec: &CompositeSpec,
    anchors: &[SourceAnchor],
    dropped: bool,
) -> Result<String, Refusal> {
    let contributors = value
        .inputs
        .iter()
        .map(|index| {
            let input = hole
                .inputs
                .get(*index)
                .ok_or_else(|| invalid("composite lost its input row"))?;
            let overlap = input
                .source
                .to_mm
                .min(value.span.to_mm)
                .saturating_sub(input.source.from_mm.max(value.span.from_mm));
            if overlap == 0 {
                return Err(invalid("composite references a non-overlapping input"));
            }
            Ok(format!(
                "{{\"input_index\":{index},\"overlap_mm\":{overlap}}}"
            ))
        })
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    let length = value.span.length_mm();
    let body = format!(
        "{{\"hole_id\":{},\"domain\":{},\"from_mm\":{},\"to_mm\":{},\"sampled_mm\":{},\"unsampled_mm\":{},\"coverage_fraction\":{},\"grade_bounds\":{},\"column\":{},\"method\":{},\"unit\":{},\"basis\":{},\"weighting\":\"sampled_length_mm\",\"weight\":{},\"dropped\":{dropped},\"contributors\":[{contributors}],\"start\":{},\"end\":{}}}",
        quoted(&hole.hole_id),
        quoted(&value.domain),
        value.span.from_mm,
        value.span.to_mm,
        value.sampled_mm,
        length - value.sampled_mm,
        value.sampled_mm as f64 / length as f64,
        bounds(value.value),
        quoted(&spec.column),
        quoted(&spec.method),
        quoted(&spec.unit),
        quoted(&format!("{:?}", spec.basis)),
        value.weight,
        position(trace, value.span.from_mm)?,
        position(trace, value.span.to_mm)?,
    );
    let representation = match value.value {
        Some(b) => Representation::Interval { lo: b.lo, hi: b.hi },
        None => Representation::Placeholder {
            token: "no_assayed_support".into(),
        },
    };
    let claim = Claim::new(
        "drill.composite/1",
        value.clone(),
        &body,
        representation,
        value.support,
        AdditivityClass::IntensiveWeighted,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        vec![],
        vec![],
        anchors.to_vec(),
    );
    claim.check_id()?;
    Ok(format!(
        "{{\"claim\":{{\"id\":{},\"metric\":{},\"evidence\":\"Assumed\",\"state\":\"Estimated\",\"calibration\":\"uncalibrated\",\"confidentiality\":\"Internal\",\"support\":{{\"kind\":\"composite\",\"length_mm\":{length}}},\"additivity\":\"IntensiveWeighted\",\"source_scope\":\"report.sources\"}},\"result\":{body}}}",
        quoted(&claim.claim_id.to_string()),
        quoted(&claim.metric_key),
    ))
}

pub(super) fn render(
    holes: &[PreparedHole],
    bundle: &AcquireExportBundle,
    anchors: &[SourceAnchor],
    manifest: &Manifest,
) -> Result<(String, String), Refusal> {
    let declaration = manifest
        .compositing
        .as_ref()
        .ok_or_else(|| invalid("missing composite declaration"))?;
    let spec = &declaration.spec;
    let columns = bundle
        .assay_report
        .resolved_columns
        .iter()
        .filter(|c| c.column_name == spec.column)
        .collect::<Vec<_>>();
    let [column] = columns.as_slice() else {
        return Err(invalid("selected assay column must occur exactly once"));
    };
    if column.undeclared_in_dialect
        || column.unit != ResolvedUnit::Registered(spec.unit.clone())
        || column.basis != Some(spec.basis)
        || column.support != Some(ColumnSupport::PointCore)
    {
        return Err(invalid(
            "composite column requires matching registered unit, basis and PointCore support; no conversion is implicit",
        ));
    }
    let composites = prepare_composites(holes, spec, &declaration.domains)?;
    let model = match manifest.survey_error_model.as_str() {
        "random" => ErrorModel::random(),
        "systematic" => ErrorModel::systematic(),
        _ => return Err(invalid("unknown survey error model")),
    };
    let mut output = Vec::new();
    let mut md = String::from(
        "| Hole | Kept / dropped | Sampled / unsampled (m) | Conservation relative error |\n|---|---:|---:|---:|\n",
    );
    for hole in composites {
        let geometry = holes
            .iter()
            .find(|h| h.collar.hole_id == hole.hole_id)
            .ok_or_else(|| invalid("composite lost its geometry"))?;
        let surveys = geometry
            .surveys
            .iter()
            .map(|s| s.record.clone())
            .collect::<Vec<_>>();
        let trace = survey_error::propagate(&geometry.collar, &surveys, &model)
            .map_err(|e| e.to_refusal())?;
        let kept = hole
            .set
            .composites
            .iter()
            .map(|c| piece(c, &hole, &trace, spec, anchors, false))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let dropped = hole
            .set
            .dropped
            .iter()
            .map(|c| piece(c, &hole, &trace, spec, anchors, true))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let sources = hole
            .inputs
            .iter()
            .map(|s| source(s, spec))
            .collect::<Vec<_>>()
            .join(",");
        let missing = hole
            .missing
            .iter()
            .map(|s| source(s, spec))
            .collect::<Vec<_>>()
            .join(",");
        let conservation = &hole.set.conservation;
        let _ = writeln!(
            md,
            "| {} | {} / {} | {:.3} / {:.3} | {} |",
            markdown_cell(&hole.hole_id),
            hole.set.composites.len(),
            hole.set.dropped.len(),
            hole.sampled_mm as f64 / 1000.0,
            hole.unsampled_mm as f64 / 1000.0,
            conservation.relative_error,
        );
        output.push(format!(
            "{{\"hole_id\":{},\"sampled_mm\":{},\"unsampled_mm\":{},\"inputs\":[{sources}],\"missing\":[{missing}],\"composites\":[{kept}],\"dropped\":[{dropped}],\"conservation\":{{\"basis\":\"assay_unit_times_mm\",\"includes_dropped\":true,\"input_total_bounds\":{},\"output_total_bounds\":{},\"relative_error\":{}}}}}",
            quoted(&hole.hole_id),
            hole.sampled_mm,
            hole.unsampled_mm,
            bounds(Some(conservation.inputs)),
            bounds(Some(conservation.composites)),
            conservation.relative_error,
        ));
    }
    md.push_str("\nGrade bounds describe censoring over sampled support; they are not statistical confidence intervals. Conservation includes explicitly dropped residuals and is in assay-unit × millimetres, not metal mass. JSON retains each composite's actual support, input contribution lengths, source observations and endpoint covariance.\n");
    let (residual, minimum) = match spec.residual {
        Residual::Keep => ("keep", "null".into()),
        Residual::Distribute => ("distribute", "null".into()),
        Residual::MergeBelow { min_mm } => ("merge_below", min_mm.to_string()),
        Residual::DropBelow { min_mm } => ("drop_below", min_mm.to_string()),
    };
    Ok((
        format!(
            "{{\"column\":{},\"method\":{},\"unit\":{},\"basis\":{},\"length_mm\":{},\"residual\":{},\"minimum_mm\":{minimum},\"weighting\":\"sampled_length_mm\",\"survey_error_model\":{},\"coordinate_order\":[\"east\",\"north\",\"up\"],\"covariance_unit\":\"m2\",\"holes\":[{}]}}",
            quoted(&spec.column),
            quoted(&spec.method),
            quoted(&spec.unit),
            quoted(&format!("{:?}", spec.basis)),
            spec.length_mm,
            quoted(residual),
            quoted(&manifest.survey_error_model),
            output.join(","),
        ),
        md,
    ))
}
