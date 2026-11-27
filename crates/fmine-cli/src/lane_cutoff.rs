//! DESIGN (BD-252): read a declared Lane scenario and explain its exact limiting grades.

use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{self, Json};
use fmine_optimize::cutoff::{
    BlendAllocation, BlendConstraints, CapacityBalance, CurveCutoff, EnergyLimit, GradeRecovery,
    GradeScreen, GradeTonnageBin, ImpurityLimit, LaneCutoffs, LaneTerms, MAX_GRADE_BINS,
    MAX_RECOVERY_KNOTS, RecoveryCurve, screen_grade,
};
use fmine_units::ExactQty;

use crate::{Envelope, NextAction, json_string as js, worksheet_input};

pub(super) const VERB: &str = "plan cutoff";
pub(super) const USAGE: &str = "plan cutoff --input <lane-scenario.json>";
const SCHEMA: &str = "fmine.lane-cutoff.v1";
const NO_CLAIM: &str = "These exact break-even grades are conditional on one declared period, one metal, constant recovery and the submitted financial and capacity assumptions. A grade screen compares only with the stated limiting case. Selecting a binding constraint, balancing cut-offs and approving an operating policy require separate evidence.";
const CURVE_NO_CLAIM: &str = "These exact results use one declared period and metal, the submitted economics, and linear interpolation of recovered metal per dry tonne between the declared grade knots. The curve supports only that closed range. Selecting a binding constraint, balancing cut-offs and approving an operating policy require separate evidence.";
pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.lane-cutoff.v1","command":"plan cutoff --input <lane-scenario.json>","basis":"One period and metal on a 100_percent_project basis. All ore is dry_metric_tonne and all metal uses the declared metal_unit (g, kg, oz_t, lb_avdp or t). All monetary terms share one currency. The recovery fraction maps contained to recovered metal sold at the declared net price.","#,
    r#""input":"Exact decimal strings for price_per_metal_unit, selling_cost_per_metal_unit, milling_cost_per_dry_tonne, fixed_cost_per_period, opportunity_cost_per_period, mill_dry_tonnes_per_period and market_metal_units_per_period. Optional feed_grade_per_dry_tonne object with lower and upper decimal strings. Evidence is assumed or synthetic.","#,
    r#""recovery":"Declare exactly one of recovery_fraction or recovery_curve. A curve has 2 to 128 ordered grade_per_dry_tonne and recovery_fraction knots using exact decimal strings. Recovered grade is grade times recovery, rises or stays flat, and is interpolated linearly. The feed band must lie entirely inside the curve. A crossing at a plateau uses the first grade.","#,
    r#""grade_tonnage":"Optional array of 1 to 128 already-mined inventory bins with increasing grade_per_dry_tonne and positive dry_tonnes as exact decimal strings. Every bin must lie within recovery support. The screen takes highest grades first and permits partial bins. Reports each physical cap alone, both caps together and the positive-contribution allocation under both caps.","#,
    r#""blend_impurity_limit":"Optional object with element and maximum_mass_fraction. Supply exactly two grade_tonnage bins with impurity_mass_fraction values in [0,1] on a dry-mass basis. Equal grades are permitted in this mode. Maximizes contribution subject to both caps and the mass-weighted impurity ceiling under fixed additive parcel recovery. Fixed and opportunity charges enter the separate Lane limiting grades.","#,
    r#""blend_energy_budget_kwh":"Optional nonnegative exact decimal total for one period. Supply exactly two grade_tonnage bins with nonnegative specific_energy_kwh_per_dry_tonne. May be supplied alone or with blend_impurity_limit. Energy is the sum of selected dry tonnes times declared specific energy for each parcel. The constrained result gives the exact budget, used and remaining kWh.","#,
    r#""output":"Separate mine-, mill- and market-limited cases. Curve reports distinguish an exact cutoff, all_supported_grades_clear, and no_supported_grade_clears. Equality clears. Retains source SHA-256, units, period and uncalibrated status.","#,
    r#""example":"tests/planning/lane_cutoff.json","curve_example":"tests/planning/lane_recovery_curve.json","grade_tonnage_example":"tests/planning/lane_grade_tonnage.json","impurity_blend_example":"tests/planning/lane_impurity_blend.json","energy_blend_example":"tests/planning/lane_energy_blend.json"}"#,
);

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

fn element<'a>(object: &'a Json, name: &str) -> Result<&'a str, Refusal> {
    let symbol = field(object, name)?;
    if symbol.len() > 3
        || !symbol.starts_with(|c: char| c.is_ascii_uppercase())
        || !symbol.bytes().skip(1).all(|c| c.is_ascii_lowercase())
    {
        return Err(declaration(format!(
            "{name} requires one case-sensitive element symbol"
        )));
    }
    Ok(symbol)
}

pub(super) fn feed_mass_limit(unit: &str) -> Result<ExactQty, Refusal> {
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

pub(super) fn exact(q: ExactQty) -> String {
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
    balance: Option<CapacityBalance>,
}

struct Inventory {
    bins: Vec<GradeTonnageBin>,
    constraints: Option<BlendConstraints>,
}

fn parse_grade_tonnage(
    raw: &Json,
    maximum: Option<ExactQty>,
    budget: Option<ExactQty>,
) -> Result<Inventory, Refusal> {
    let rows = raw
        .as_array()
        .ok_or_else(|| declaration("grade_tonnage requires an array"))?;
    if !(1..=MAX_GRADE_BINS).contains(&rows.len()) {
        return Err(declaration(format!(
            "grade_tonnage requires 1 to {MAX_GRADE_BINS} bins"
        )));
    }
    let expected_fields = 2 + usize::from(maximum.is_some()) + usize::from(budget.is_some());
    let bins = rows.iter()
        .map(|row| {
            if row.as_object().is_none_or(|fields| fields.len() != expected_fields) {
                return Err(declaration(
                    "each bin requires grade_per_dry_tonne and dry_tonnes, plus impurity_mass_fraction or specific_energy_kwh_per_dry_tonne when its corresponding blend limit is declared",
                ));
            }
            Ok(GradeTonnageBin {
                grade: quantity(row, "grade_per_dry_tonne")?,
                dry_tonnes: quantity(row, "dry_tonnes")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if maximum.is_none() && budget.is_none() {
        return Ok(Inventory {
            bins,
            constraints: None,
        });
    }
    let [low, high] = rows else {
        return Err(declaration("a constrained blend requires exactly two bins"));
    };
    let pair = |field| Ok::<_, Refusal>([quantity(low, field)?, quantity(high, field)?]);
    let impurity = maximum
        .map(|maximum_mass_fraction| {
            Ok::<_, Refusal>(ImpurityLimit {
                maximum_mass_fraction,
                bin_mass_fractions: pair("impurity_mass_fraction")?,
            })
        })
        .transpose()?;
    let energy = budget
        .map(|budget_kwh| {
            Ok::<_, Refusal>(EnergyLimit {
                budget_kwh,
                bin_kwh_per_dry_tonne: pair("specific_energy_kwh_per_dry_tonne")?,
            })
        })
        .transpose()?;
    Ok(Inventory {
        bins,
        constraints: Some(BlendConstraints { impurity, energy }),
    })
}

fn evaluate_cutoffs(
    doc: &Json,
    terms: LaneTerms,
    max_grade: ExactQty,
    grades: Option<(ExactQty, ExactQty)>,
    impurity_maximum: Option<ExactQty>,
    energy_budget: Option<ExactQty>,
) -> Result<ScenarioCutoffs, Refusal> {
    let bins = doc
        .get("grade_tonnage")
        .map(|raw| parse_grade_tonnage(raw, impurity_maximum, energy_budget))
        .transpose()?;
    if (impurity_maximum.is_some() || energy_budget.is_some()) && bins.is_none() {
        return Err(declaration("blend limits require grade_tonnage"));
    }
    let screen = |recovery| {
        bins.as_ref()
            .map(|inventory| match inventory.constraints {
                Some(limits) => terms.balance_feed_with_constraints(
                    &inventory.bins,
                    max_grade,
                    recovery,
                    limits,
                ),
                None => terms.balance_feed(&inventory.bins, max_grade, recovery),
            })
            .transpose()
    };
    match (doc.get("recovery_fraction"), doc.get("recovery_curve")) {
        (Some(_), None) => {
            let recovery = quantity(doc, "recovery_fraction")?;
            let c = terms.evaluate_constant(recovery)?;
            let balance = screen(GradeRecovery::Constant(recovery))?;
            // A break-even grade above one tonne of metal per dry tonne is one no ore can carry.
            let bounded = |grade: ExactQty| -> Result<CurveCutoff, Refusal> {
                let above = grade
                    .checked_cmp(max_grade)
                    .map_err(|_| {
                        Refusal::new(
                            RefusalCode::NumericalCertificateFailed207,
                            "break-even grade comparison exceeds exact arithmetic range",
                        )
                    })?
                    .is_gt();
                Ok(if above {
                    CurveCutoff::NoSupportedGradeClears
                } else {
                    CurveCutoff::AtGrade(grade)
                })
            };
            Ok(ScenarioCutoffs {
                limits: LaneCutoffs {
                    mine: bounded(c.mine)?,
                    mill: bounded(c.mill)?,
                    market: bounded(c.market)?,
                },
                support: None,
                balance,
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
            let balance = screen(GradeRecovery::Curve(&curve))?;
            Ok(ScenarioCutoffs {
                limits: terms.evaluate_curve(&curve)?,
                support: Some(curve.support()),
                balance,
            })
        }
        _ => Err(declaration(
            "declare exactly one of recovery_fraction and recovery_curve",
        )),
    }
}

fn render_balance(
    balance: CapacityBalance,
    terms: LaneTerms,
    unit: &str,
    currency: &str,
    markdown: &mut String,
) -> String {
    let feed = exact(balance.available_dry_tonnes);
    let metal = exact(balance.available_recovered_metal);
    let mill = exact(terms.mill_dry_tonnes_per_period);
    let market = exact(terms.market_metal_units_per_period);
    let contribution = exact(balance.incremental_contribution);
    let _ = writeln!(
        markdown,
        "\n## Grade-tonnage screen\n\nAvailable: **{feed} dry tonnes** with **{metal} {unit}** recoverable. Mill cap **{mill} dry tonnes**, market cap **{market} {unit}**.\n\nThe screen takes the highest grades first and permits a partial boundary bin. The positive-contribution row applies both caps to already-mined feed and leaves break-even feed idle. Its incremental contribution is **{contribution} {currency}** after selling and variable milling costs. It excludes fixed and opportunity charges.\n\n| Screen | Dry tonnes | Recovered {unit} | Lowest selected grade | Share at that grade |\n|---|---:|---:|---:|---:|"
    );
    let mut rows = Vec::new();
    for (key, caption, allocation, economic) in [
        ("mill_only", "Mill only", balance.mill_only, false),
        ("market_only", "Market only", balance.market_only, false),
        ("both", "Mill and market", balance.both, false),
        (
            "positive_contribution",
            "Positive contribution under both caps",
            balance.positive_contribution_feed,
            true,
        ),
    ] {
        let tonnes = exact(allocation.dry_tonnes);
        let metal = exact(allocation.recovered_metal);
        let (grade, share) = allocation.boundary.map_or_else(
            || ("—".into(), "—".into()),
            |boundary| (exact(boundary.grade), exact(boundary.fraction)),
        );
        let _ = writeln!(
            markdown,
            "| {caption} | {tonnes} | {metal} | {grade} | {share} |"
        );
        let boundary = if allocation.boundary.is_some() {
            format!(
                "{{\"grade_per_dry_tonne\":{},\"selected_fraction\":{}}}",
                js(&grade),
                js(&share)
            )
        } else {
            "null".into()
        };
        let money = if economic {
            format!(",\"incremental_contribution\":{}", js(&contribution))
        } else {
            String::new()
        };
        rows.push(format!(
            "\"{key}\":{{\"dry_tonnes\":{},\"recovered_metal\":{},\"boundary\":{boundary}{money}}}",
            js(&tonnes),
            js(&metal)
        ));
    }
    format!(
        ",\"capacity_balance\":{{\"policy\":\"highest_grade_first\",\"available_dry_tonnes\":{},\"available_recovered_metal\":{},\"mill_dry_tonnes_cap\":{},\"market_recovered_metal_cap\":{},{}}}",
        js(&feed),
        js(&metal),
        js(&mill),
        js(&market),
        rows.join(",")
    )
}

fn render_blend(
    blend: BlendAllocation,
    impurity: Option<(&str, ExactQty)>,
    unit: &str,
    currency: &str,
    markdown: &mut String,
) -> String {
    let dry = exact(blend.dry_tonnes);
    let metal = exact(blend.recovered_metal);
    let contribution = exact(blend.incremental_contribution);
    let (key, title, assay_text, assay_json, assumptions) = if let Some((element, maximum)) =
        impurity
    {
        let maximum = exact(maximum);
        let assay = blend.impurity_fraction.map(exact);
        let text = format!(
            "{element} dry-mass fraction **{}** with ceiling **{maximum}**. ",
            assay.as_deref().unwrap_or("—")
        );
        let assay = assay.map_or_else(|| "null".into(), |value| js(&value));
        let json = format!(
            "\"element\":{},\"maximum_mass_fraction\":{},\"blended_mass_fraction\":{assay},",
            js(element),
            js(&maximum)
        );
        (
            "impurity_constrained_blend",
            if blend.energy.is_some() {
                "Impurity- and energy-constrained blend"
            } else {
                "Impurity-constrained blend"
            },
            text,
            json,
            "The blend uses fixed per-bin assays and additive recovered metal. A zero-tonne selection has no blended assay.",
        )
    } else {
        (
            "energy_constrained_blend",
            "Energy-constrained blend",
            String::new(),
            String::new(),
            "The blend uses additive recovered metal.",
        )
    };
    let (energy_text, energy_json) = blend.energy.map_or_else(
        || (String::new(), String::new()),
        |energy| {
            let budget = exact(energy.budget_kwh);
            let used = exact(energy.used_kwh);
            let remaining = exact(energy.remaining_kwh);
            (
                format!("\n\nGrinding uses **{used} kWh** of **{budget} kWh**, leaving **{remaining} kWh**. Each parcel uses its declared specific energy per dry tonne."),
                format!(",\"energy\":{{\"budget_kwh\":{},\"used_kwh\":{},\"remaining_kwh\":{}}}", js(&budget), js(&used), js(&remaining)),
            )
        },
    );
    let _ = writeln!(
        markdown,
        "\n## {title}\n\n{assay_text}Select **{dry} dry tonnes** for **{metal} {unit}** and **{contribution} {currency}** of incremental contribution. {assumptions}{energy_text}\n\n| Feed grade ({unit}/dry t) | Selected dry tonnes |\n|---:|---:|"
    );
    let bins = blend.selected_bins.map(|bin| {
        let grade = exact(bin.grade);
        let tonnes = exact(bin.dry_tonnes);
        let _ = writeln!(markdown, "| {grade} | {tonnes} |");
        format!(
            "{{\"grade_per_dry_tonne\":{},\"selected_dry_tonnes\":{}}}",
            js(&grade),
            js(&tonnes)
        )
    });
    format!(
        ",\"{key}\":{{{assay_json}\"dry_tonnes\":{},\"recovered_metal\":{},\"incremental_contribution\":{},\"selected_bins\":[{}]{energy_json}}}",
        js(&dry),
        js(&metal),
        js(&contribution),
        bins.join(",")
    )
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
        "grade_tonnage",
        "blend_impurity_limit",
        "blend_energy_budget_kwh",
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
    let metal = element(&doc, "metal")?;
    let metal_unit = field(&doc, "metal_unit")?;
    let mass_basis = field(&doc, "ore_mass_basis")?;
    let ownership = field(&doc, "ownership_basis")?;
    let evidence = field(&doc, "evidence")?;
    if currency.len() != 3 || !currency.bytes().all(|c| c.is_ascii_uppercase()) {
        return Err(declaration(
            "currency requires one three-letter uppercase code",
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
    let impurity = doc
        .get("blend_impurity_limit")
        .map(|limit| {
            if limit.as_object().is_none_or(|fields| fields.len() != 2) {
                return Err(declaration(
                    "blend_impurity_limit requires exactly element and maximum_mass_fraction",
                ));
            }
            let symbol = element(limit, "element")?;
            if symbol == metal {
                return Err(declaration(
                    "impurity element must differ from the recovered metal",
                ));
            }
            Ok((symbol, quantity(limit, "maximum_mass_fraction")?))
        })
        .transpose()?;
    let energy_budget = doc
        .get("blend_energy_budget_kwh")
        .map(|_| quantity(&doc, "blend_energy_budget_kwh"))
        .transpose()?;
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
        balance,
    } = evaluate_cutoffs(
        &doc,
        terms,
        max_grade,
        grades,
        impurity.map(|(_, maximum)| maximum),
        energy_budget,
    )?;
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
    let balance_json = balance.map_or_else(String::new, |balance| {
        render_balance(balance, terms, metal_unit, currency, &mut markdown)
    });
    let blend_json = balance
        .and_then(|balance| balance.constrained_blend)
        .map_or_else(String::new, |blend| {
            render_blend(blend, impurity, metal_unit, currency, &mut markdown)
        });
    let _ = writeln!(markdown, "\n{no_claim}");
    let payload = format!(
        "{{\"schema\":\"{SCHEMA}\",\"period\":{},\"currency\":{},\"metal\":{},\"metal_unit\":{},\"ore_mass_basis\":{},\"ownership_basis\":{},\"input_evidence\":{},\"calibration\":\"uncalibrated\",\"source\":{source_json},\"feed_grade_per_dry_tonne\":{grade_json}{curve_json},\"limiting_cutoffs\":[{}]{balance_json}{blend_json}}}",
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
