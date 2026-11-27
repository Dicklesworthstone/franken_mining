//! DESIGN (BD-252): source-bound one-period ore routing from a declared worksheet.
//! Determinism: Exact

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::Path,
};

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{self, Json};
use fmine_optimize::cutoff::routing::{
    self, Allocation, MAX_PARCELS, MAX_ROUTES, Parcel, Route, RoutePair, Scenario,
};
use fmine_units::ExactQty;

use crate::{
    Envelope, NextAction, json_string as js,
    lane_cutoff::{exact, feed_mass_limit},
    worksheet_input::{self, markdown_cell},
};

pub(super) const VERB: &str = "plan blend";
pub(super) const USAGE: &str = "plan blend --input <routing-worksheet.json>";
const SCHEMA: &str = "fmine.route-blend.v1";
const SCOPE: &str = "One period of already-mined, divisible feed with fixed per-pair recovery, additive specific energy and declared variable costs. The allocation is conditional on these uncalibrated inputs.";
pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.route-blend.v1","example":"tests/planning/route_blend.json","basis":"One period, metal mass unit, currency and 100_percent_project basis. Ore mass is dry_metric_tonne. Evidence is synthetic or assumed. Quantities are exact decimal strings.","#,
    r#""parcels":"1 to 8 with unique id, available_dry_tonnes and grade_per_dry_tonne. An optional common impurity element requires impurity_mass_fraction for every parcel.","#,
    r#""routes":"1 to 4 with unique id, dry_tonnes_cap, milling_cost_per_dry_tonne, optional energy_budget_kwh and impurity_ceiling. The pairs list contains only eligible parcels, with parcel id, recovery_fraction and haul_cost_per_dry_tonne. An energy budget requires specific_energy_kwh_per_dry_tonne for every eligible pair on that route.","#,
    r#""market":"One shared market_recovered_metal_cap, price_per_metal_unit and selling_cost_per_metal_unit. Objective is recovered metal at net price less milling and haul costs. Idle feed is feasible."}"#,
);

fn declaration(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, detail)
}

fn object(value: &Json, allowed: &[&str]) -> Result<(), Refusal> {
    let pairs = value
        .as_object()
        .ok_or_else(|| declaration("expected a worksheet object"))?;
    if let Some((key, _)) = pairs
        .iter()
        .find(|(key, _)| !allowed.contains(&key.as_str()))
    {
        return Err(declaration(format!("unknown route-blend field {key}")));
    }
    Ok(())
}

fn field<'a>(value: &'a Json, key: &str) -> Result<&'a str, Refusal> {
    value
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| declaration(format!("{key} requires a string")))
}

fn label<'a>(value: &'a Json, key: &str) -> Result<&'a str, Refusal> {
    let text = field(value, key)?;
    if text.trim().is_empty() || text.len() > 128 || text.chars().any(char::is_control) {
        return Err(declaration(format!(
            "{key} requires a nonempty printable label of at most 128 bytes"
        )));
    }
    Ok(text)
}

fn quantity(value: &Json, key: &str) -> Result<ExactQty, Refusal> {
    ExactQty::from_decimal_str(field(value, key)?).map_err(|_| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("{key} requires an exact decimal within range"),
        )
    })
}

fn optional(value: &Json, key: &str) -> Result<Option<ExactQty>, Refusal> {
    value.get(key).map(|_| quantity(value, key)).transpose()
}

fn rows<'a>(value: &'a Json, key: &str, maximum: usize) -> Result<&'a [Json], Refusal> {
    let rows = value
        .get(key)
        .and_then(Json::as_array)
        .ok_or_else(|| declaration(format!("{key} requires an array")))?;
    if rows.len() > maximum {
        return Err(declaration(format!("{key} permits at most {maximum} rows")));
    }
    Ok(rows)
}

fn q(value: ExactQty) -> String {
    js(&exact(value))
}

fn maybe(value: Option<ExactQty>) -> String {
    value.map_or_else(|| "null".into(), q)
}

fn allocation(a: Allocation) -> String {
    format!(
        "\"dry_tonnes\":{},\"recovered_metal\":{},\"incremental_contribution\":{}",
        q(a.dry_tonnes),
        q(a.recovered_metal),
        q(a.contribution)
    )
}

fn evaluate(text: &str, source: &SourceAnchor) -> Result<Envelope, Refusal> {
    let doc = json::parse(text.as_bytes()).map_err(declaration)?;
    object(
        &doc,
        &[
            "schema",
            "period",
            "currency",
            "metal",
            "metal_unit",
            "ore_mass_basis",
            "ownership_basis",
            "evidence",
            "impurity",
            "market_recovered_metal_cap",
            "price_per_metal_unit",
            "selling_cost_per_metal_unit",
            "parcels",
            "routes",
        ],
    )?;
    if field(&doc, "schema")? != SCHEMA {
        return Err(declaration(format!("expected {SCHEMA}")));
    }
    if field(&doc, "ore_mass_basis")? != "dry_metric_tonne"
        || field(&doc, "ownership_basis")? != "100_percent_project"
    {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "route blend uses dry metric ore tonnes and a 100% project basis",
        ));
    }
    let period = label(&doc, "period")?;
    let currency = field(&doc, "currency")?;
    let metal = label(&doc, "metal")?;
    let unit = field(&doc, "metal_unit")?;
    let evidence = field(&doc, "evidence")?;
    if currency.len() != 3
        || !currency.bytes().all(|c| c.is_ascii_uppercase())
        || !matches!(evidence, "synthetic" | "assumed")
    {
        return Err(declaration(
            "declare an uppercase three-letter currency and synthetic or assumed evidence",
        ));
    }
    let impurity = doc
        .get("impurity")
        .map(|_| label(&doc, "impurity"))
        .transpose()?;
    if impurity == Some(metal) {
        return Err(declaration("impurity must differ from the recovered metal"));
    }
    let parcel_rows = rows(&doc, "parcels", MAX_PARCELS)?;
    let mut parcel_index = BTreeMap::new();
    let mut parcels = Vec::new();
    let mut parcel_ids = Vec::new();
    for row in parcel_rows {
        object(
            row,
            &[
                "id",
                "available_dry_tonnes",
                "grade_per_dry_tonne",
                "impurity_mass_fraction",
            ],
        )?;
        let id = label(row, "id")?;
        if parcel_index.insert(id, parcels.len()).is_some() {
            return Err(declaration(format!("duplicate parcel {id}")));
        }
        let assay = optional(row, "impurity_mass_fraction")?;
        if assay.is_some() != impurity.is_some() {
            return Err(declaration(
                "declare an impurity assay for every parcel when an impurity is named",
            ));
        }
        parcels.push(Parcel {
            available_dry_tonnes: quantity(row, "available_dry_tonnes")?,
            grade_per_dry_tonne: quantity(row, "grade_per_dry_tonne")?,
            impurity_mass_fraction: assay,
        });
        parcel_ids.push(id);
    }
    let mut route_ids = Vec::new();
    let mut route_names = BTreeSet::new();
    let mut routes = Vec::new();
    let mut pairs = Vec::new();
    for row in rows(&doc, "routes", MAX_ROUTES)? {
        object(
            row,
            &[
                "id",
                "dry_tonnes_cap",
                "milling_cost_per_dry_tonne",
                "energy_budget_kwh",
                "impurity_ceiling",
                "pairs",
            ],
        )?;
        let id = label(row, "id")?;
        if !route_names.insert(id) {
            return Err(declaration(format!("duplicate route {id}")));
        }
        let route = routes.len();
        route_ids.push(id);
        routes.push(Route {
            dry_tonnes_cap: quantity(row, "dry_tonnes_cap")?,
            milling_cost_per_dry_tonne: quantity(row, "milling_cost_per_dry_tonne")?,
            energy_budget_kwh: optional(row, "energy_budget_kwh")?,
            impurity_ceiling: optional(row, "impurity_ceiling")?,
        });
        for entry in rows(row, "pairs", MAX_PARCELS)? {
            object(
                entry,
                &[
                    "parcel",
                    "recovery_fraction",
                    "haul_cost_per_dry_tonne",
                    "specific_energy_kwh_per_dry_tonne",
                ],
            )?;
            let id = label(entry, "parcel")?;
            let parcel = *parcel_index
                .get(id)
                .ok_or_else(|| declaration(format!("unknown parcel {id}")))?;
            pairs.push(RoutePair {
                parcel,
                route,
                recovery_fraction: quantity(entry, "recovery_fraction")?,
                haul_cost_per_dry_tonne: quantity(entry, "haul_cost_per_dry_tonne")?,
                specific_energy_kwh_per_dry_tonne: optional(
                    entry,
                    "specific_energy_kwh_per_dry_tonne",
                )?,
            });
        }
    }
    let market = quantity(&doc, "market_recovered_metal_cap")?;
    let result = routing::solve(Scenario {
        parcels: &parcels,
        routes: &routes,
        pairs: &pairs,
        physical_grade_limit: feed_mass_limit(unit)?,
        market_recovered_metal_cap: market,
        price_per_metal_unit: quantity(&doc, "price_per_metal_unit")?,
        selling_cost_per_metal_unit: quantity(&doc, "selling_cost_per_metal_unit")?,
    })?;

    let mut markdown = format!(
        "# One-period ore routing\n\n{} · {} · {unit} · {currency} · 100% project basis\n\nInputs are {evidence} and uncalibrated. Source SHA-256 `{}`.\n\n**{} dry tonnes** recover **{} {unit}** and contribute **{} {currency}** against a shared market cap of **{} {unit}**.\n\n| Parcel | Destination | Dry tonnes | Recovered {unit} | Contribution ({currency}) |\n|---|---|---:|---:|---:|\n",
        markdown_cell(period),
        markdown_cell(metal),
        fmine_core::Digest32(source.artifact_sha256),
        exact(result.total.dry_tonnes),
        exact(result.total.recovered_metal),
        exact(result.total.contribution),
        exact(market)
    );
    let mut allocations = Vec::new();
    for (pair, selected) in pairs.iter().zip(result.pairs) {
        let (parcel, route) = (parcel_ids[pair.parcel], route_ids[pair.route]);
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} |",
            markdown_cell(parcel),
            markdown_cell(route),
            exact(selected.dry_tonnes),
            exact(selected.recovered_metal),
            exact(selected.contribution)
        );
        allocations.push(format!(
            "{{\"parcel\":{},\"route\":{},{} }}",
            js(parcel),
            js(route),
            allocation(selected)
        ));
    }
    let assay_title = impurity.map_or_else(|| "Impurity".into(), markdown_cell);
    let _ = writeln!(
        markdown,
        "\n| Destination | Dry tonnes used / cap | Grinding kWh used / cap | {assay_title} fraction / cap |\n|---|---:|---:|---:|"
    );
    let mut route_results = Vec::new();
    let cell = |quantity: Option<ExactQty>| quantity.map_or_else(|| "—".into(), exact);
    for ((id, route), total) in route_ids.into_iter().zip(routes).zip(result.routes) {
        let _ = writeln!(
            markdown,
            "| {} | {} / {} | {} / {} | {} / {} |",
            markdown_cell(id),
            exact(total.allocation.dry_tonnes),
            exact(route.dry_tonnes_cap),
            cell(total.energy_used_kwh),
            cell(route.energy_budget_kwh),
            cell(total.blended_impurity_mass_fraction),
            cell(route.impurity_ceiling)
        );
        route_results.push(format!("{{\"route\":{},{},\"dry_tonnes_cap\":{},\"energy_used_kwh\":{},\"energy_budget_kwh\":{},\"blended_impurity_mass_fraction\":{},\"impurity_ceiling\":{}}}", js(id), allocation(total.allocation), q(route.dry_tonnes_cap), maybe(total.energy_used_kwh), maybe(route.energy_budget_kwh), maybe(total.blended_impurity_mass_fraction), maybe(route.impurity_ceiling)));
    }
    markdown.push_str("\n| Parcel | Unallocated dry tonnes |\n|---|---:|\n");
    let mut remaining = Vec::new();
    for (id, dry) in parcel_ids.into_iter().zip(result.unallocated_dry_tonnes) {
        let _ = writeln!(markdown, "| {} | {} |", markdown_cell(id), exact(dry));
        remaining.push(format!(
            "{{\"parcel\":{},\"unallocated_dry_tonnes\":{}}}",
            js(id),
            q(dry)
        ));
    }
    let _ = writeln!(markdown, "\n{SCOPE}");
    let payload = format!(
        "{{\"schema\":\"{SCHEMA}\",\"period\":{},\"currency\":{},\"metal\":{},\"metal_unit\":{},\"ore_mass_basis\":\"dry_metric_tonne\",\"ownership_basis\":\"100_percent_project\",\"input_evidence\":{},\"calibration\":\"uncalibrated\",\"impurity\":{},\"source\":{},\"market_recovered_metal_cap\":{},\"total\":{{{}}},\"allocations\":[{}],\"routes\":[{}],\"parcels\":[{}]}}",
        js(period),
        js(currency),
        js(metal),
        js(unit),
        js(evidence),
        impurity.map_or_else(|| "null".into(), js),
        worksheet_input::source_json(source, text.len()),
        q(market),
        allocation(result.total),
        allocations.join(","),
        route_results.join(","),
        remaining.join(",")
    );
    let mut envelope = Envelope::ok(VERB, payload, vec![]);
    envelope.no_claim = Some(SCOPE);
    envelope.rendered_markdown = Some(markdown);
    Ok(envelope)
}

pub(super) fn execute(path: &Path, root: &Path) -> Envelope {
    worksheet_input::read(&root.join(path), "route blend worksheet", 64 * 1024)
        .and_then(|(text, source)| evaluate(&text, &source))
        .unwrap_or_else(|refusal| {
            Envelope::refused(
                VERB,
                refusal,
                vec![NextAction::new(
                    "fmine robot-docs",
                    "Review the ore-routing worksheet fields.",
                )],
            )
        })
}
