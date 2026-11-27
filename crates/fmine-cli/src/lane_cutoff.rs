//! DESIGN (BD-252): read a declared Lane scenario and explain its exact limiting grades.

use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{self, Json};
use fmine_optimize::cutoff::{
    CurveCutoff, GradeScreen, LaneCutoffs, LaneTerms, MAX_RECOVERY_KNOTS, RecoveryCurve,
    screen_grade,
};
use fmine_units::ExactQty;

use crate::{Envelope, NextAction, json_string as js, worksheet_input};

pub(super) const VERB: &str = "plan cutoff";
pub(super) const USAGE: &str = "plan cutoff --input <lane-scenario.json>";
const SCHEMA: &str = "fmine.lane-cutoff.v1";
const NO_CLAIM: &str = "These exact break-even grades are conditional on one declared period, one metal, constant recovery and the submitted financial and capacity assumptions. A grade screen compares only with the stated limiting case. Selecting a binding constraint, balancing cut-offs and approving an operating policy require separate evidence.";
const CURVE_NO_CLAIM: &str = "These exact results use one declared period and metal, the submitted economics, and linear interpolation of recovered metal per dry tonne between the declared grade knots. The curve supports only that closed range. Selecting a binding constraint, balancing cut-offs and approving an operating policy require separate evidence.";
pub(super) const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.lane-cutoff.v1","command":"plan cutoff --input <lane-scenario.json>","basis":"One period and metal on a 100_percent_project basis. All ore is dry_metric_tonne and all metal uses the declared metal_unit (g, kg, oz_t, lb_avdp or t). All monetary terms share one currency. The recovery fraction maps contained to recovered metal sold at the declared net price.","input":"Exact decimal strings for price_per_metal_unit, selling_cost_per_metal_unit, milling_cost_per_dry_tonne, fixed_cost_per_period, opportunity_cost_per_period, mill_dry_tonnes_per_period and market_metal_units_per_period; optional feed_grade_per_dry_tonne object with lower and upper decimal strings. Evidence is assumed or synthetic.","recovery":"Declare exactly one of recovery_fraction or recovery_curve. A curve has 2 to 128 ordered grade_per_dry_tonne and recovery_fraction knots using exact decimal strings. Recovered grade is grade times recovery, rises or stays flat, and is interpolated linearly. The feed band must lie entirely inside the curve. A crossing at a plateau uses the first grade.","output":"Separate mine-, mill- and market-limited cases. Curve reports distinguish an exact cutoff, all_supported_grades_clear, and no_supported_grade_clears. Equality clears. Retains source SHA-256, units, period and uncalibrated status.","example":"tests/planning/lane_cutoff.json","curve_example":"tests/planning/lane_recovery_curve.json"}"#;

const ECONOMIC_FIELDS: [&str; 8] = [
    "price_per_metal_unit",
    "selling_cost_per_metal_unit",
    "recovery_fraction",
    "milling_cost_per_dry_tonne",
    "fixed_cost_per_period",
    "opportunity_cost_per_period",
    "mill_dry_tonnes_per_period",
    "market_metal_units_per_period",
];

fn declaration(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, detail)
}

fn field<'a>(object: &'a Json, name: &str) -> Result<&'a str, Refusal> {
    object
        .get(name)
        .and_then(Json::as_str)
        .ok_or_else(|| declaration(format!("Lane scenario requires a string field {name}")))
}

fn quantity(object: &Json, name: &str) -> Result<ExactQty, Refusal> {
    ExactQty::from_decimal_str(field(object, name)?).map_err(|_| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("{name} requires an exact decimal within the supported range"),
        )
    })
}

fn feed_mass_limit(unit: &str) -> Result<ExactQty, Refusal> {
    let grams = ExactQty::from_integer(1_000_000);
    let limit = match unit {
        "g" => Ok(grams),
        "kg" => Ok(ExactQty::from_integer(1_000)),
        "oz_t" => grams.grams_to_troy_oz(),
        "lb_avdp" => grams.grams_to_avdp_lb(),
        "t" => Ok(ExactQty::from_integer(1)),
        _ => {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                "unsupported metal mass unit",
            ));
        }
    };
    limit.map_err(|_| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "dry tonne limit exceeds exact unit conversion range",
        )
    })
}

fn exact(q: ExactQty) -> String {
    format!("{}/{}", q.num(), q.den())
}

fn input_help() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "See the Lane scenario fields and supported units before changing an assumption.",
    )]
}

struct ScenarioCutoffs {
    limits: LaneCutoffs<CurveCutoff>,
    support: Option<(ExactQty, ExactQty)>,
}

fn evaluate_cutoffs(
    doc: &Json,
    terms: LaneTerms,
    max_grade: ExactQty,
    grades: Option<(ExactQty, ExactQty)>,
) -> Result<ScenarioCutoffs, Refusal> {
    match (doc.get("recovery_fraction"), doc.get("recovery_curve")) {
        (Some(_), None) => {
            let c = terms.evaluate_constant(quantity(doc, "recovery_fraction")?)?;
            Ok(ScenarioCutoffs {
                limits: LaneCutoffs {
                    mine: CurveCutoff::AtGrade(c.mine),
                    mill: CurveCutoff::AtGrade(c.mill),
                    market: CurveCutoff::AtGrade(c.market),
                },
                support: None,
            })
        }
        (None, Some(raw)) => {
            let knots = raw
                .as_array()
                .ok_or_else(|| declaration("recovery_curve requires an array"))?;
            if !(2..=MAX_RECOVERY_KNOTS).contains(&knots.len()) {
                return Err(declaration(format!(
                    "recovery_curve requires 2 to {MAX_RECOVERY_KNOTS} knots"
                )));
            }
            let points = knots.iter().map(|knot| {
                if knot.as_object().is_none_or(|fields| fields.len() != 2) {
                    return Err(declaration("each recovery knot requires exactly grade_per_dry_tonne and recovery_fraction"));
                }
                Ok((quantity(knot, "grade_per_dry_tonne")?, quantity(knot, "recovery_fraction")?))
            }).collect::<Result<Vec<_>, Refusal>>()?;
            let curve = RecoveryCurve::new(&points, max_grade)?;
            if let Some((lower, upper)) = grades {
                curve.check_band(lower, upper)?;
            }
            Ok(ScenarioCutoffs {
                limits: terms.evaluate_curve(&curve)?,
                support: Some(curve.support()),
            })
        }
        _ => Err(declaration(
            "declare exactly one of recovery_fraction and recovery_curve",
        )),
    }
}

fn evaluate(text: &str, source: &fmine_core::anchor::SourceAnchor) -> Result<Envelope, Refusal> {
    let doc = json::parse(text.as_bytes()).map_err(declaration)?;
    let declared_fields = [
        "schema",
        "period",
        "currency",
        "metal",
        "metal_unit",
        "ore_mass_basis",
        "ownership_basis",
        "evidence",
        "feed_grade_per_dry_tonne",
        "recovery_curve",
    ];
    let members = doc
        .as_object()
        .ok_or_else(|| declaration("Lane scenario must be a JSON object"))?;
    for (key, _) in members {
        if !declared_fields.contains(&key.as_str()) && !ECONOMIC_FIELDS.contains(&key.as_str()) {
            return Err(declaration(format!("unknown Lane scenario field {key}")));
        }
    }
    if field(&doc, "schema")? != SCHEMA {
        return Err(declaration(format!("expected schema {SCHEMA}")));
    }
    let period = field(&doc, "period")?;
    if period.trim().is_empty() || period.len() > 128 || period.chars().any(char::is_control) {
        return Err(declaration(
            "period requires a nonempty printable label of at most 128 bytes",
        ));
    }
    let currency = field(&doc, "currency")?;
    let metal = field(&doc, "metal")?;
    let metal_unit = field(&doc, "metal_unit")?;
    let mass_basis = field(&doc, "ore_mass_basis")?;
    let ownership = field(&doc, "ownership_basis")?;
    let evidence = field(&doc, "evidence")?;
    if currency.len() != 3 || !currency.bytes().all(|c| c.is_ascii_uppercase()) {
        return Err(declaration(
            "currency requires one three-letter uppercase code",
        ));
    }
    if metal.len() > 3
        || !metal.starts_with(|c: char| c.is_ascii_uppercase())
        || !metal.bytes().skip(1).all(|c| c.is_ascii_lowercase())
    {
        return Err(declaration(
            "metal requires one case-sensitive element symbol",
        ));
    }
    if mass_basis != "dry_metric_tonne" || ownership != "100_percent_project" {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "use dry metric ore tonnes and a 100% project basis",
        ));
    }
    let max_grade = feed_mass_limit(metal_unit)?;
    if !matches!(evidence, "assumed" | "synthetic") {
        return Err(declaration(
            "declare assumed or synthetic Lane scenario inputs",
        ));
    }
    let grades = doc
        .get("feed_grade_per_dry_tonne")
        .map(|band| {
            if band.as_object().is_none_or(|fields| fields.len() != 2) {
                return Err(declaration("feed grade requires exactly lower and upper"));
            }
            let (lower, upper) = (quantity(band, "lower")?, quantity(band, "upper")?);
            if upper.checked_cmp(max_grade).map_err(|_| Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "feed-grade mass comparison exceeds exact arithmetic range",
            ))?.is_gt() {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "feed-grade upper bound exceeds one tonne of contained metal per dry tonne of ore",
                ));
            }
            Ok((lower, upper))
        })
        .transpose()?;
    let terms = LaneTerms {
        price_per_metal_unit: quantity(&doc, "price_per_metal_unit")?,
        selling_cost_per_metal_unit: quantity(&doc, "selling_cost_per_metal_unit")?,
        milling_cost_per_dry_tonne: quantity(&doc, "milling_cost_per_dry_tonne")?,
        fixed_cost_per_period: quantity(&doc, "fixed_cost_per_period")?,
        opportunity_cost_per_period: quantity(&doc, "opportunity_cost_per_period")?,
        mill_dry_tonnes_per_period: quantity(&doc, "mill_dry_tonnes_per_period")?,
        market_metal_units_per_period: quantity(&doc, "market_metal_units_per_period")?,
    };
    let ScenarioCutoffs {
        limits: cutoffs,
        support,
    } = evaluate_cutoffs(&doc, terms, max_grade, grades)?;
    let source_json = worksheet_input::source_json(source, text.len());
    let mut markdown = format!(
        "# Lane limiting cut-off grades\n\n{} · {metal} · {metal_unit}/dry metric tonne · {currency} · 100% project basis\n\nInputs are {evidence} and uncalibrated. Source SHA-256 `{}`.\n\n",
        worksheet_input::markdown_cell(period),
        fmine_core::Digest32(source.artifact_sha256),
    );
    let grade_json = grades.map_or_else(
        || "null".into(),
        |(lower, upper)| {
            let (lower, upper) = (exact(lower), exact(upper));
            let _ = writeln!(
                markdown,
                "Declared feed-grade band **{lower} to {upper}**. Equality clears a threshold.\n"
            );
            format!("{{\"lower\":{},\"upper\":{}}}", js(&lower), js(&upper))
        },
    );
    let (curve_json, no_claim) = if let Some((lower, upper)) = support {
        let (lower, upper) = (exact(lower), exact(upper));
        let _ = writeln!(
            markdown,
            "Recovery curve spans **{lower} to {upper} {metal_unit}/dry metric tonne**.\n"
        );
        (
            format!(
                ",\"recovery_model\":\"piecewise_linear_recovered_grade\",\"recovery_curve_support\":{{\"lower\":{},\"upper\":{}}}",
                js(&lower),
                js(&upper)
            ),
            CURVE_NO_CLAIM,
        )
    } else {
        (String::new(), NO_CLAIM)
    };
    markdown.push_str(
        "| Limiting constraint | Break-even grade (exact) | Feed-grade band |\n|---|---:|---|\n",
    );
    let mut rows = Vec::new();
    for (case, limit) in [
        ("mine", cutoffs.mine),
        ("mill", cutoffs.mill),
        ("market", cutoffs.market),
    ] {
        let (grade_json, grade_cell, grade_status) = match limit {
            CurveCutoff::AtGrade(grade) => {
                let value = exact(grade);
                (js(&value), value, "within_support")
            }
            CurveCutoff::AllSupportedGradesClear => (
                "null".into(),
                "All supported grades clear".into(),
                "all_supported_grades_clear",
            ),
            CurveCutoff::NoSupportedGradeClears => (
                "null".into(),
                "No supported grade clears".into(),
                "no_supported_grade_clears",
            ),
        };
        let status = grades
            .map(|(lower, upper)| match limit {
                CurveCutoff::AtGrade(grade) => screen_grade(lower, upper, grade),
                CurveCutoff::AllSupportedGradesClear => Ok(GradeScreen::EntireBandClears),
                CurveCutoff::NoSupportedGradeClears => Ok(GradeScreen::EntireBandBelow),
            })
            .transpose()?;
        let (json, label) = match status {
            Some(GradeScreen::EntireBandClears) => ("\"entire_band_clears\"", "Entire band clears"),
            Some(GradeScreen::EntireBandBelow) => ("\"entire_band_below\"", "Entire band below"),
            Some(GradeScreen::Straddles) => ("\"straddles\"", "Straddles"),
            None => ("null", "No band supplied"),
        };
        let _ = writeln!(markdown, "| {case} | {grade_cell} | {label} |");
        let status_json = support.map_or_else(String::new, |_| {
            format!(",\"cutoff_status\":{}", js(grade_status))
        });
        rows.push(format!(
            "{{\"limiting_constraint\":{},\"cutoff_grade\":{grade_json},\"feed_grade_screen\":{json}{status_json}}}",
            js(case),
        ));
    }
    let _ = writeln!(markdown, "\n{no_claim}");
    let payload = format!(
        "{{\"schema\":\"{SCHEMA}\",\"period\":{},\"currency\":{},\"metal\":{},\"metal_unit\":{},\"ore_mass_basis\":{},\"ownership_basis\":{},\"input_evidence\":{},\"calibration\":\"uncalibrated\",\"source\":{source_json},\"feed_grade_per_dry_tonne\":{grade_json}{curve_json},\"limiting_cutoffs\":[{}]}}",
        js(period),
        js(currency),
        js(metal),
        js(metal_unit),
        js(mass_basis),
        js(ownership),
        js(evidence),
        rows.join(","),
    );
    let mut envelope = Envelope::ok(VERB, payload, input_help());
    envelope.no_claim = Some(no_claim);
    envelope.rendered_markdown = Some(markdown);
    Ok(envelope)
}

pub(super) fn execute(path: &Path, root: &Path) -> Envelope {
    worksheet_input::read(&root.join(path), "Lane cut-off scenario", 64 * 1024)
        .and_then(|(text, source)| evaluate(&text, &source))
        .unwrap_or_else(|refusal| Envelope::refused(VERB, refusal, input_help()))
}
