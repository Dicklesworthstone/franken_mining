//! DESIGN (BD-252): source-bound ore routing and stockpile carry from a declared worksheet.
//! Determinism: Exact

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::Path,
};

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{self, Json};
use fmine_optimize::cutoff::routing::{
    self, Allocation, AssayWindow, Horizon, MAX_ASSAYS, MAX_PARCELS, MAX_PERIODS, MAX_ROUTES,
    Parcel, Period, Route, RoutePair, Routing, Scenario,
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
const HORIZON_SCHEMA: &str = "fmine.route-blend-horizon.v1";
const FEASIBILITY_SCHEMA: &str = "fmine.route-blend-feasibility.v1";
const MAX_CAPACITY_OPTIONS: usize = 8;
const MAX_PACKAGE_CHANGES: usize = 4;
const CAPACITY_TARGET_FIELDS: [&str; 3] = ["period", "route", "parcel"];
const SCOPE: &str = "One period of already-mined, divisible feed with fixed per-pair recovery, additive specific energy and declared variable costs. The allocation is conditional on these uncalibrated inputs.";
const HORIZON_SCOPE: &str = "The worksheet carries fixed-grade parcels through at most four periods. Declared arrivals enter before routing, and storage caps apply at period close. Contribution uses declared discounts, fixed per-pair recovery, additive specific energy and variable costs.";
const CAMPAIGN_SCOPE: &str = "The horizon compares two mutually exclusive modes of one circuit over up to four periods. Both modes use fixed per-pair recovery and declared assay windows. A mode change at a period start pays its declared cost at that period's discount, even when the circuit receives no ore. The worksheet assumes changeovers fit between periods and supplies no operating authorization.";
const CAPACITY_SCOPE: &str = "Each option separately increases one declared cap and reoptimizes the full horizon. The quoted cost is already discounted in the worksheet currency and project basis. Option values apply to the declared increase under the submitted assumptions.";
const PACKAGE_SCOPE: &str = "Each option reoptimizes the same horizon. A package applies all its declared changes together and pays one quoted cost. Costs are already discounted in the worksheet currency and project basis. All values are conditional on the submitted assumptions.";
const FEASIBILITY_SCOPE: &str = "Each quote is evaluated separately against the declared physical limits and commitments. For feasible quotes, discounted contribution includes campaign changeovers when declared, and the quoted present-value cost is charged once. The base horizon is infeasible, so incremental value and break-even cost are unavailable.";
pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.route-blend.v1","example":"tests/planning/route_blend.json","basis":"One period, metal mass unit, currency and 100_percent_project basis. Ore mass is dry_metric_tonne. Evidence is synthetic or assumed. Quantities are exact decimal strings.","#,
    r#""parcels":"1 to 8 with unique id, available_dry_tonnes and grade_per_dry_tonne. An optional common impurity element requires impurity_mass_fraction for every parcel.","#,
    r#""routes":"1 to 4 with unique id, dry_tonnes_cap, milling_cost_per_dry_tonne, optional energy_budget_kwh and impurity_ceiling. The pairs list contains only eligible parcels, with parcel id, recovery_fraction and haul_cost_per_dry_tonne. An energy budget requires specific_energy_kwh_per_dry_tonne for every eligible pair on that route.","#,
    r#""market":"One shared market_recovered_metal_cap, optional nonnegative market_recovered_metal_minimum in the same metal unit, price_per_metal_unit and selling_cost_per_metal_unit. Both limits apply to total recovered metal within that period. Objective is recovered metal at net price less milling and haul costs.","#,
    r#""assay_windows":{"example":"tests/planning/route_blend_assay_windows.json","declaration":"Optional analytes array of one to three distinct constituents. Each parcel supplies every named dry-mass fraction in assays. Each route may declare assay_windows keyed by name, with min, max, or both as exact decimal mass fractions. Use this form in either schema. It is exclusive with the single-impurity fields."},"#,
    r#""horizon":{"schema":"fmine.route-blend-horizon.v1","example":"tests/planning/route_blend_horizon.json","periods":"Replace the root period, market, price, selling and routes fields with an ordered periods array, containing up to four objects with id, discount_factor in [0,1], arrivals (parcel and dry_tonnes rows), routes and the market and economic fields. Each parcel can declare stockpile_cap_dry_tonnes. Closing stock carries on the same parcel and basis; caps apply at each close.", "#,
    r#""campaign":{"example":"tests/planning/route_blend_campaign.json","declaration":"Optional horizon campaign with exactly two route_modes named in every period, an initial_mode from that pair, and nonnegative switch_cost in nominal worksheet currency. Exactly one mode is available each period. Mode transitions are discounted in the receiving period. Capacity options reoptimize the mode schedule."},"#,
    r#""capacity_options":{"example":"tests/planning/route_blend_capacity_options.json","declaration":"Optional array of 1 to 8 independent options, each with unique id, kind, positive increase and nonnegative present_value_cost in the worksheet currency. Use period and route for route_throughput (dry metric tonnes) or route_energy (kWh), period for market (recovered metal unit), or parcel for stockpile (dry metric tonnes at every period close). Energy and stockpile increases require a declared finite base cap. An infeasible base horizon returns fmine.route-blend-feasibility.v1. See tests/planning/route_blend_delivery_commitment.json.", "#,
    r#""package_example":"tests/planning/route_blend_capacity_packages.json","packages":"Use an option with only id, present_value_cost and a changes array of two to four distinct cap targets. Each change supplies kind, increase and its target fields. All changes enter one reoptimization and the quoted package cost is charged once."}}}"#,
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
    if !valid_label(text) {
        return Err(declaration(format!(
            "{key} requires a nonempty printable label of at most 128 bytes"
        )));
    }
    Ok(text)
}

fn valid_label(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 128 && !text.chars().any(char::is_control)
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

#[derive(Clone)]
struct DeclaredPeriod<'a> {
    id: &'a str,
    route_ids: Vec<&'a str>,
    routes: Vec<Route>,
    pairs: Vec<RoutePair>,
    arrivals: Vec<ExactQty>,
    discount: ExactQty,
    market: ExactQty,
    market_minimum: Option<ExactQty>,
    price: ExactQty,
    selling: ExactQty,
}

impl DeclaredPeriod<'_> {
    fn model(&self) -> Period<'_> {
        Period {
            routes: &self.routes,
            pairs: &self.pairs,
            arrivals_dry_tonnes: &self.arrivals,
            discount_factor: self.discount,
            market_recovered_metal_cap: self.market,
            market_recovered_metal_minimum: self.market_minimum,
            price_per_metal_unit: self.price,
            selling_cost_per_metal_unit: self.selling,
        }
    }

    fn describe_commitment(&self, markdown: &mut String, unit: &str) -> String {
        self.market_minimum.map_or_else(String::new, |minimum| {
            let _ = writeln!(
                markdown,
                "Recovered-metal minimum **{} {unit}**.\n",
                exact(minimum)
            );
            format!(",\"market_recovered_metal_minimum\":{}", q(minimum))
        })
    }
}

#[derive(Clone, Copy)]
struct Campaign<'a> {
    modes: [&'a str; 2],
    initial: usize,
    switch_cost: ExactQty,
}

struct CampaignChoice<'a> {
    campaign: Campaign<'a>,
    path: Vec<usize>,
    switches: usize,
    discounted_switch_cost: ExactQty,
    net: ExactQty,
    paths: usize,
    feasible_paths: usize,
}

struct HorizonEvaluation<'a> {
    routing: routing::HorizonRouting,
    campaign: Option<CampaignChoice<'a>>,
}

impl HorizonEvaluation<'_> {
    fn net(&self) -> ExactQty {
        self.campaign
            .as_ref()
            .map_or(self.routing.discounted_contribution, |choice| choice.net)
    }
}

fn read_campaign(value: &Json) -> Result<Campaign<'_>, Refusal> {
    object(value, &["route_modes", "initial_mode", "switch_cost"])?;
    let [first, second] = rows(value, "route_modes", 2)? else {
        return Err(declaration("a campaign requires exactly two route modes"));
    };
    let modes = [first, second].map(|entry| entry.as_str().filter(|name| valid_label(name)));
    let [Some(first), Some(second)] = modes else {
        return Err(declaration(
            "campaign route modes require printable route ids",
        ));
    };
    if first == second {
        return Err(declaration("campaign route modes must be distinct"));
    }
    let modes = [first, second];
    let initial_mode = label(value, "initial_mode")?;
    let initial = modes
        .iter()
        .position(|name| *name == initial_mode)
        .ok_or_else(|| declaration("initial_mode must identify one of the campaign routes"))?;
    let switch_cost = quantity(value, "switch_cost")?;
    if switch_cost.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "campaign switch_cost must be nonnegative",
        ));
    }
    Ok(Campaign {
        modes,
        initial,
        switch_cost,
    })
}

fn campaign_arithmetic(_: fmine_units::UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        "campaign contribution or switch cost exceeds exact arithmetic range",
    )
}

fn solve_declared_horizon<'a>(
    horizon: Horizon<'_>,
    periods: &[DeclaredPeriod<'_>],
    campaign: Option<Campaign<'a>>,
) -> Result<Option<HorizonEvaluation<'a>>, Refusal> {
    let Some(campaign) = campaign else {
        return Ok(
            routing::try_solve_horizon(horizon)?.map(|routing| HorizonEvaluation {
                routing,
                campaign: None,
            }),
        );
    };
    let mut indices = Vec::with_capacity(periods.len());
    for period in periods {
        let route_index = |mode| {
            period
                .route_ids
                .iter()
                .position(|name| *name == mode)
                .ok_or_else(|| {
                    declaration(format!("campaign route {mode} is missing in {}", period.id))
                })
        };
        indices.push([
            route_index(campaign.modes[0])?,
            route_index(campaign.modes[1])?,
        ]);
    }
    let zero = ExactQty::from_integer(0);
    let paths = 1 << periods.len();
    let mut feasible_paths = 0;
    let mut best: Option<(routing::HorizonRouting, CampaignChoice<'_>)> = None;
    for mask in 0..paths {
        let path: Vec<_> = (0..periods.len())
            .map(|index| (mask >> (periods.len() - 1 - index)) & 1)
            .collect();
        let mut routes = Vec::with_capacity(periods.len());
        for ((period, route_indices), &mode) in periods.iter().zip(&indices).zip(&path) {
            let mut variants = period.routes.clone();
            let inactive_cap = &mut variants[route_indices[1 - mode]].dry_tonnes_cap;
            if inactive_cap.signum() > 0 {
                *inactive_cap = zero;
            }
            routes.push(variants);
        }
        let variants: Vec<_> = horizon
            .periods
            .iter()
            .zip(&routes)
            .map(|(&period, routes)| Period { routes, ..period })
            .collect();
        let Some(routing) = routing::try_solve_horizon(Horizon {
            periods: &variants,
            ..horizon
        })?
        else {
            continue;
        };
        feasible_paths += 1;
        let mut previous = campaign.initial;
        let mut switches = 0;
        let mut discounted_switch_cost = zero;
        for (period, &mode) in periods.iter().zip(&path) {
            if mode != previous {
                let cost = campaign
                    .switch_cost
                    .checked_mul(period.discount)
                    .map_err(campaign_arithmetic)?;
                discounted_switch_cost = discounted_switch_cost
                    .checked_add(cost)
                    .map_err(campaign_arithmetic)?;
                switches += 1;
            }
            previous = mode;
        }
        let net = routing
            .discounted_contribution
            .checked_sub(discounted_switch_cost)
            .map_err(campaign_arithmetic)?;
        if let Some((_, incumbent)) = &best {
            let order = net
                .checked_cmp(incumbent.net)
                .map_err(campaign_arithmetic)?;
            if order.is_lt() || (order.is_eq() && switches >= incumbent.switches) {
                continue;
            }
        }
        best = Some((
            routing,
            CampaignChoice {
                campaign,
                path,
                switches,
                discounted_switch_cost,
                net,
                paths,
                feasible_paths: 0,
            },
        ));
    }
    let Some((routing, mut choice)) = best else {
        return Ok(None);
    };
    choice.feasible_paths = feasible_paths;
    Ok(Some(HorizonEvaluation {
        routing,
        campaign: Some(choice),
    }))
}

fn horizon_infeasible(periods: &[DeclaredPeriod<'_>], campaign: bool) -> Refusal {
    let alternatives = if campaign {
        "no campaign mode sequence"
    } else {
        "no routing"
    };
    let limits = if periods.iter().any(|period| period.market_minimum.is_some()) {
        "declared caps and recovered-metal commitments"
    } else {
        "declared stockpile and processing caps"
    };
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("{alternatives} can satisfy the {limits}"),
    )
}

impl CampaignChoice<'_> {
    fn path_label(&self) -> String {
        self.path
            .iter()
            .map(|&mode| markdown_cell(self.campaign.modes[mode]))
            .collect::<Vec<_>>()
            .join(" → ")
    }

    fn json(&self, periods: &[DeclaredPeriod<'_>], gross: ExactQty) -> String {
        let path = periods
            .iter()
            .zip(&self.path)
            .map(|(period, &mode)| {
                format!(
                    "{{\"period\":{},\"mode\":{}}}",
                    js(period.id),
                    js(self.campaign.modes[mode])
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"initial_mode\":{},\"switch_cost\":{},\"switches\":{},\"gross_discounted_contribution\":{},\"discounted_switch_cost\":{},\"net_discounted_contribution\":{},\"enumerated_paths\":{},\"feasible_paths\":{},\"period_modes\":[{path}]}}",
            js(self.campaign.modes[self.campaign.initial]),
            q(self.campaign.switch_cost),
            self.switches,
            q(gross),
            q(self.discounted_switch_cost),
            q(self.net),
            self.paths,
            self.feasible_paths
        )
    }
}

struct Report<'a> {
    currency: &'a str,
    metal: &'a str,
    unit: &'a str,
    evidence: &'a str,
    impurity: Option<&'a str>,
    assay_names: &'a [&'a str],
    source: &'a SourceAnchor,
    bytes: usize,
}

impl Report<'_> {
    fn basis_json(&self) -> String {
        let mut basis = format!(
            "\"currency\":{},\"metal\":{},\"metal_unit\":{},\"ore_mass_basis\":\"dry_metric_tonne\",\"ownership_basis\":\"100_percent_project\",\"input_evidence\":{},\"calibration\":\"uncalibrated\",\"impurity\":{},\"source\":{}",
            js(self.currency),
            js(self.metal),
            js(self.unit),
            js(self.evidence),
            self.impurity.map_or_else(|| "null".into(), js),
            worksheet_input::source_json(self.source, self.bytes)
        );
        if self.named_assays() {
            let names = self
                .assay_names
                .iter()
                .map(|name| js(name))
                .collect::<Vec<_>>()
                .join(",");
            let _ = write!(basis, ",\"analytes\":[{names}]");
        }
        basis
    }

    fn named_assays(&self) -> bool {
        self.impurity.is_none() && !self.assay_names.is_empty()
    }
}

fn assay_names<'a>(
    doc: &'a Json,
    metal: &str,
    impurity: Option<&'a str>,
) -> Result<Vec<&'a str>, Refusal> {
    if doc.get("analytes").is_none() {
        return Ok(impurity.into_iter().collect());
    }
    if impurity.is_some() {
        return Err(declaration("use analytes or impurity for the worksheet"));
    }
    let names = rows(doc, "analytes", MAX_ASSAYS)?;
    if names.is_empty() {
        return Err(declaration("analytes requires at least one constituent"));
    }
    let mut seen = BTreeSet::from([metal.to_lowercase()]);
    names
        .iter()
        .map(|value| {
            let name = value
                .as_str()
                .filter(|text| valid_label(text))
                .ok_or_else(|| declaration("analytes requires printable constituent names"))?;
            if !seen.insert(name.to_lowercase()) {
                return Err(declaration(format!(
                    "duplicate or recovered-metal constituent {name}"
                )));
            }
            Ok(name)
        })
        .collect()
}

fn read_assays(row: &Json, report: &Report<'_>) -> Result<Vec<ExactQty>, Refusal> {
    if report.named_assays() {
        if row.get("impurity_mass_fraction").is_some() {
            return Err(declaration("named assays exclude impurity_mass_fraction"));
        }
        let assays = row
            .get("assays")
            .ok_or_else(|| declaration("each parcel requires the named assays"))?;
        object(assays, report.assay_names)?;
        return report
            .assay_names
            .iter()
            .map(|name| quantity(assays, name))
            .collect();
    }
    if row.get("assays").is_some() {
        return Err(declaration(
            "parcel assays require the analytes declaration",
        ));
    }
    let assay = optional(row, "impurity_mass_fraction")?;
    if assay.is_some() != report.impurity.is_some() {
        return Err(declaration(
            "declare an impurity assay for every parcel when an impurity is named",
        ));
    }
    Ok(assay.into_iter().collect())
}

fn route_windows(row: &Json, report: &Report<'_>) -> Result<Vec<AssayWindow>, Refusal> {
    if report.named_assays() {
        if row.get("impurity_ceiling").is_some() {
            return Err(declaration("named assays exclude impurity_ceiling"));
        }
        let Some(windows) = row.get("assay_windows") else {
            return Ok(vec![AssayWindow::default(); report.assay_names.len()]);
        };
        object(windows, report.assay_names)?;
        return report
            .assay_names
            .iter()
            .map(|name| {
                let Some(window) = windows.get(name) else {
                    return Ok(AssayWindow::default());
                };
                object(window, &["min", "max"])?;
                let min = optional(window, "min")?;
                let max = optional(window, "max")?;
                if min.is_none() && max.is_none() {
                    return Err(declaration(format!(
                        "assay window for {name} requires a min or max"
                    )));
                }
                Ok(AssayWindow { min, max })
            })
            .collect();
    }
    if row.get("assay_windows").is_some() {
        return Err(declaration(
            "assay_windows requires the analytes declaration",
        ));
    }
    let max = optional(row, "impurity_ceiling")?;
    if report.impurity.is_some() {
        return Ok(vec![AssayWindow { min: None, max }]);
    }
    if max.is_some() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "route impurity ceiling requires the common impurity assay",
        ));
    }
    Ok(Vec::new())
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
            "analytes",
            "market_recovered_metal_cap",
            "market_recovered_metal_minimum",
            "price_per_metal_unit",
            "selling_cost_per_metal_unit",
            "parcels",
            "routes",
            "periods",
            "capacity_options",
            "campaign",
        ],
    )?;
    let horizon = match field(&doc, "schema")? {
        SCHEMA => false,
        HORIZON_SCHEMA => true,
        _ => {
            return Err(declaration(format!(
                "expected {SCHEMA} or {HORIZON_SCHEMA}"
            )));
        }
    };
    if horizon
        && [
            "period",
            "routes",
            "price_per_metal_unit",
            "selling_cost_per_metal_unit",
            "market_recovered_metal_cap",
            "market_recovered_metal_minimum",
        ]
        .into_iter()
        .any(|key| doc.get(key).is_some())
    {
        return Err(declaration(
            "a routing horizon declares route and economic fields within each period",
        ));
    }
    if !horizon && doc.get("periods").is_some() {
        return Err(declaration("a periods array requires the horizon schema"));
    }
    if !horizon && doc.get("capacity_options").is_some() {
        return Err(declaration("capacity options require the horizon schema"));
    }
    if !horizon && doc.get("campaign").is_some() {
        return Err(declaration("campaign modes require the horizon schema"));
    }
    if field(&doc, "ore_mass_basis")? != "dry_metric_tonne"
        || field(&doc, "ownership_basis")? != "100_percent_project"
    {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "route blend uses dry metric ore tonnes and a 100% project basis",
        ));
    }
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
    let names = assay_names(&doc, metal, impurity)?;
    let report = Report {
        currency,
        metal,
        unit,
        evidence,
        impurity,
        assay_names: &names,
        source,
        bytes: text.len(),
    };
    let parcel_rows = rows(&doc, "parcels", MAX_PARCELS)?;
    let mut parcel_index = BTreeMap::new();
    let mut parcels = Vec::new();
    let mut parcel_ids = Vec::new();
    let mut caps = Vec::new();
    for row in parcel_rows {
        object(
            row,
            &[
                "id",
                "available_dry_tonnes",
                "grade_per_dry_tonne",
                "impurity_mass_fraction",
                "assays",
                "stockpile_cap_dry_tonnes",
            ],
        )?;
        let id = label(row, "id")?;
        if parcel_index.insert(id, parcels.len()).is_some() {
            return Err(declaration(format!("duplicate parcel {id}")));
        }
        parcels.push(Parcel {
            available_dry_tonnes: quantity(row, "available_dry_tonnes")?,
            grade_per_dry_tonne: quantity(row, "grade_per_dry_tonne")?,
            assays: read_assays(row, &report)?,
        });
        let cap = optional(row, "stockpile_cap_dry_tonnes")?;
        if cap.is_some() && !horizon {
            return Err(declaration("stockpile caps require the horizon schema"));
        }
        caps.push(cap);
        parcel_ids.push(id);
    }
    let physical_grade_limit = feed_mass_limit(unit)?;
    if horizon {
        let mut period_ids = BTreeSet::new();
        let mut periods = Vec::new();
        for row in rows(&doc, "periods", MAX_PERIODS)? {
            object(
                row,
                &[
                    "id",
                    "discount_factor",
                    "arrivals",
                    "routes",
                    "market_recovered_metal_cap",
                    "market_recovered_metal_minimum",
                    "price_per_metal_unit",
                    "selling_cost_per_metal_unit",
                ],
            )?;
            let id = label(row, "id")?;
            if !period_ids.insert(id) {
                return Err(declaration(format!("duplicate period {id}")));
            }
            periods.push(read_period(row, id, &parcel_index, true, &report)?);
        }
        let models: Vec<_> = periods.iter().map(DeclaredPeriod::model).collect();
        let horizon = Horizon {
            parcels: &parcels,
            periods: &models,
            stockpile_caps_dry_tonnes: &caps,
            physical_grade_limit,
        };
        let campaign = doc.get("campaign").map(read_campaign).transpose()?;
        let result = solve_declared_horizon(horizon, &periods, campaign)?;
        let options = if doc.get("capacity_options").is_some() {
            Some(evaluate_capacity(
                rows(&doc, "capacity_options", MAX_CAPACITY_OPTIONS)?,
                horizon,
                &periods,
                &parcel_index,
                report.unit,
                campaign,
            )?)
        } else {
            None
        };
        let Some(result) = result else {
            let options =
                options.ok_or_else(|| horizon_infeasible(&periods, campaign.is_some()))?;
            return render_capacity_rescue(&report, &periods, options, campaign.is_some());
        };
        let options = options
            .map(|options| {
                compare_capacity(&report, &periods, options, result.net(), campaign.is_some())
            })
            .transpose()?;
        return render_horizon(
            &report,
            &parcel_ids,
            &parcels,
            &caps,
            &periods,
            result,
            options,
        );
    }

    let period = read_period(&doc, label(&doc, "period")?, &parcel_index, false, &report)?;
    let result = routing::solve(Scenario {
        parcels: &parcels,
        routes: &period.routes,
        pairs: &period.pairs,
        physical_grade_limit,
        market_recovered_metal_cap: period.market,
        market_recovered_metal_minimum: period.market_minimum,
        price_per_metal_unit: period.price,
        selling_cost_per_metal_unit: period.selling,
    })?;
    Ok(render_single(&report, &parcel_ids, &period, &result))
}

fn read_period<'a>(
    value: &'a Json,
    id: &'a str,
    parcel_index: &BTreeMap<&str, usize>,
    horizon: bool,
    report: &Report<'_>,
) -> Result<DeclaredPeriod<'a>, Refusal> {
    let mut route_ids = Vec::new();
    let mut route_names = BTreeSet::new();
    let mut routes = Vec::new();
    let mut pairs = Vec::new();
    for row in rows(value, "routes", MAX_ROUTES)? {
        object(
            row,
            &[
                "id",
                "dry_tonnes_cap",
                "milling_cost_per_dry_tonne",
                "energy_budget_kwh",
                "impurity_ceiling",
                "assay_windows",
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
            assay_windows: route_windows(row, report)?,
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
    let mut arrivals = Vec::new();
    let discount = if horizon {
        arrivals.resize(parcel_index.len(), ExactQty::from_integer(0));
        let mut seen = BTreeSet::new();
        for row in rows(value, "arrivals", MAX_PARCELS)? {
            object(row, &["parcel", "dry_tonnes"])?;
            let name = label(row, "parcel")?;
            let parcel = *parcel_index
                .get(name)
                .ok_or_else(|| declaration(format!("unknown arrival parcel {name}")))?;
            if !seen.insert(parcel) {
                return Err(declaration(format!("duplicate arrival for {name}")));
            }
            arrivals[parcel] = quantity(row, "dry_tonnes")?;
        }
        quantity(value, "discount_factor")?
    } else {
        ExactQty::from_integer(1)
    };
    Ok(DeclaredPeriod {
        id,
        route_ids,
        routes,
        pairs,
        arrivals,
        discount,
        market: quantity(value, "market_recovered_metal_cap")?,
        market_minimum: optional(value, "market_recovered_metal_minimum")?,
        price: quantity(value, "price_per_metal_unit")?,
        selling: quantity(value, "selling_cost_per_metal_unit")?,
    })
}

fn render_single(
    report: &Report<'_>,
    parcel_ids: &[&str],
    period: &DeclaredPeriod<'_>,
    result: &Routing,
) -> Envelope {
    let Report {
        currency,
        metal,
        unit,
        evidence,
        source,
        ..
    } = *report;

    let mut markdown = format!(
        "# One-period ore routing\n\n{} · {} · {unit} · {currency} · 100% project basis\n\nInputs are {evidence} and uncalibrated. Source SHA-256 `{}`.\n\n**{} dry tonnes** recover **{} {unit}** and contribute **{} {currency}** against a shared market cap of **{} {unit}**.\n\n",
        markdown_cell(period.id),
        markdown_cell(metal),
        fmine_core::Digest32(source.artifact_sha256),
        exact(result.total.dry_tonnes),
        exact(result.total.recovered_metal),
        exact(result.total.contribution),
        exact(period.market)
    );
    let commitment = period.describe_commitment(&mut markdown, unit);
    let (allocations, routes) = tables(report, parcel_ids, period, result, &mut markdown);
    markdown.push_str("\n| Parcel | Unallocated dry tonnes |\n|---|---:|\n");
    let mut remaining = Vec::new();
    for (&id, &dry) in parcel_ids.iter().zip(&result.unallocated_dry_tonnes) {
        let _ = writeln!(markdown, "| {} | {} |", markdown_cell(id), exact(dry));
        remaining.push(format!(
            "{{\"parcel\":{},\"unallocated_dry_tonnes\":{}}}",
            js(id),
            q(dry)
        ));
    }
    let _ = writeln!(markdown, "\n{SCOPE}");
    let payload = format!(
        "{{\"schema\":\"{SCHEMA}\",\"period\":{},{},\"market_recovered_metal_cap\":{}{commitment},\"total\":{{{}}},\"allocations\":[{allocations}],\"routes\":[{routes}],\"parcels\":[{}]}}",
        js(period.id),
        report.basis_json(),
        q(period.market),
        allocation(result.total),
        remaining.join(",")
    );
    response(payload, markdown, SCOPE)
}

fn tables(
    report: &Report<'_>,
    parcel_ids: &[&str],
    period: &DeclaredPeriod<'_>,
    result: &Routing,
    markdown: &mut String,
) -> (String, String) {
    let Report {
        unit,
        currency,
        impurity,
        ..
    } = *report;
    let DeclaredPeriod {
        pairs,
        route_ids,
        routes,
        ..
    } = period;
    let _ = writeln!(
        markdown,
        "| Parcel | Destination | Dry tonnes | Recovered {unit} | Contribution ({currency}) |\n|---|---|---:|---:|---:|"
    );
    let mut allocations = Vec::new();
    for (pair, &selected) in pairs.iter().zip(&result.pairs) {
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
    let named = report.named_assays();
    let mut assay_table = String::new();
    if named {
        markdown.push_str("\n| Destination | Dry tonnes used / cap | Grinding kWh used / cap |\n|---|---:|---:|\n");
        assay_table.push_str("\n| Destination | Constituent | Blended dry-mass fraction | Minimum | Maximum |\n|---|---|---:|---:|---:|\n");
    } else {
        let assay_title = impurity.map_or_else(|| "Impurity".into(), markdown_cell);
        let _ = writeln!(
            markdown,
            "\n| Destination | Dry tonnes used / cap | Grinding kWh used / cap | {assay_title} fraction / cap |\n|---|---:|---:|---:|"
        );
    }
    let mut route_results = Vec::new();
    let cell = |quantity: Option<ExactQty>| quantity.map_or_else(|| "—".into(), exact);
    for ((&id, route), total) in route_ids.iter().zip(routes).zip(&result.routes) {
        let blended = total.blended_assays.first().copied().flatten();
        let ceiling = route.assay_windows.first().and_then(|window| window.max);
        let mut legacy_cell = String::new();
        let assay_json = if named {
            let mut assays = Vec::new();
            for ((&name, &value), &window) in report
                .assay_names
                .iter()
                .zip(&total.blended_assays)
                .zip(&route.assay_windows)
            {
                let _ = writeln!(
                    assay_table,
                    "| {} | {} | {} | {} | {} |",
                    markdown_cell(id),
                    markdown_cell(name),
                    cell(value),
                    cell(window.min),
                    cell(window.max)
                );
                assays.push(format!(
                    "{{\"analyte\":{},\"blended_mass_fraction\":{},\"min_mass_fraction\":{},\"max_mass_fraction\":{}}}",
                    js(name), maybe(value), maybe(window.min), maybe(window.max)));
            }
            format!("\"assays\":[{}]", assays.join(","))
        } else {
            let _ = write!(legacy_cell, " {} / {} |", cell(blended), cell(ceiling));
            format!(
                "\"blended_impurity_mass_fraction\":{},\"impurity_ceiling\":{}",
                maybe(blended),
                maybe(ceiling)
            )
        };
        let _ = writeln!(
            markdown,
            "| {} | {} / {} | {} / {} |{legacy_cell}",
            markdown_cell(id),
            exact(total.allocation.dry_tonnes),
            exact(route.dry_tonnes_cap),
            cell(total.energy_used_kwh),
            cell(route.energy_budget_kwh),
        );
        route_results.push(format!("{{\"route\":{},{},\"dry_tonnes_cap\":{},\"energy_used_kwh\":{},\"energy_budget_kwh\":{},{assay_json}}}", js(id), allocation(total.allocation), q(route.dry_tonnes_cap), maybe(total.energy_used_kwh), maybe(route.energy_budget_kwh)));
    }
    markdown.push_str(&assay_table);
    (allocations.join(","), route_results.join(","))
}

fn render_horizon(
    report: &Report<'_>,
    parcel_ids: &[&str],
    parcels: &[Parcel],
    caps: &[Option<ExactQty>],
    periods: &[DeclaredPeriod<'_>],
    evaluation: HorizonEvaluation<'_>,
    options: Option<CapacityComparison>,
) -> Result<Envelope, Refusal> {
    let Report {
        currency,
        metal,
        unit,
        evidence,
        source,
        ..
    } = *report;
    let net = evaluation.net();
    let HorizonEvaluation {
        routing: result,
        campaign,
    } = evaluation;
    let mut markdown = format!(
        "# Ore routing and stockpile carry\n\n{} · {unit} · {currency} · 100% project basis\n\nInputs are {evidence} and uncalibrated. Source SHA-256 `{}`.\n\n**{} dry tonnes** recover **{} {unit}** and contribute **{} {currency}** discounted across the declared periods.\n",
        markdown_cell(metal),
        fmine_core::Digest32(source.artifact_sha256),
        exact(result.total.dry_tonnes),
        exact(result.total.recovered_metal),
        exact(net)
    );
    let (campaign_json, scope) = if let Some(choice) = &campaign {
        let _ = writeln!(
            markdown,
            "\n**Circuit mode by period** {}\n\n{} changeovers cost **{} {currency}** discounted. Gross discounted routing contribution is **{} {currency}**. Only the selected mode can receive feed in each period. Evaluated {} feasible paths out of {}.",
            choice.path_label(),
            choice.switches,
            exact(choice.discounted_switch_cost),
            exact(result.discounted_contribution),
            choice.feasible_paths,
            choice.paths
        );
        (
            format!(
                ",\"campaign\":{}",
                choice.json(periods, result.discounted_contribution)
            ),
            CAMPAIGN_SCOPE,
        )
    } else {
        (String::new(), HORIZON_SCOPE)
    };
    let mut opening: Vec<_> = parcels.iter().map(|p| p.available_dry_tonnes).collect();
    let mut period_json = Vec::new();
    let arithmetic = |_| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "stockpile report exceeds exact arithmetic range",
        )
    };
    for (period, routing) in periods.iter().zip(&result.periods) {
        let discounted = routing
            .total
            .contribution
            .checked_mul(period.discount)
            .map_err(arithmetic)?;
        let _ = writeln!(
            markdown,
            "\n## {}\n\nRecovered **{} {unit}** of the **{} {unit}** shared market cap. Contribution **{} {currency}**, discounted **{} {currency}**.\n",
            markdown_cell(period.id),
            exact(routing.total.recovered_metal),
            exact(period.market),
            exact(routing.total.contribution),
            exact(discounted)
        );
        let commitment = period.describe_commitment(&mut markdown, unit);
        let (allocations, routes) = tables(report, parcel_ids, period, routing, &mut markdown);
        markdown.push_str("\n| Parcel | Opening dry t | Arriving dry t | Processed dry t | Closing dry t | Closing cap dry t |\n|---|---:|---:|---:|---:|---:|\n");
        let mut stock = Vec::new();
        for (k, id) in parcel_ids.iter().enumerate() {
            let available = opening[k];
            let arrival = period.arrivals[k];
            let closing = routing.unallocated_dry_tonnes[k];
            let processed = available
                .checked_add(arrival)
                .and_then(|total| total.checked_sub(closing))
                .map_err(arithmetic)?;
            let cap = caps[k];
            let _ = writeln!(
                markdown,
                "| {} | {} | {} | {} | {} | {} |",
                markdown_cell(id),
                exact(available),
                exact(arrival),
                exact(processed),
                exact(closing),
                cap.map_or_else(|| "—".into(), exact)
            );
            stock.push(format!("{{\"parcel\":{},\"opening_dry_tonnes\":{},\"arriving_dry_tonnes\":{},\"processed_dry_tonnes\":{},\"closing_dry_tonnes\":{},\"stockpile_cap_dry_tonnes\":{}}}",
                js(id), q(available), q(arrival), q(processed), q(closing), maybe(cap)));
            opening[k] = closing;
        }
        period_json.push(format!("{{\"period\":{},\"discount_factor\":{},\"market_recovered_metal_cap\":{}{commitment},\"discounted_contribution\":{},\"total\":{{{}}},\"allocations\":[{allocations}],\"routes\":[{routes}],\"parcels\":[{}]}}",
            js(period.id), q(period.discount), q(period.market), q(discounted), allocation(routing.total), stock.join(",")));
    }
    let mut capacity = String::new();
    if let Some(options) = options {
        markdown.push_str(&options.markdown);
        let _ = write!(capacity, ",\"capacity_comparison\":{}", options.json);
    }
    let _ = writeln!(markdown, "\n{scope}");
    let payload = format!(
        "{{\"schema\":\"{HORIZON_SCHEMA}\",{},\"total\":{{{},\"discounted_contribution\":{}}},\"periods\":[{}]{campaign_json}{capacity}}}",
        report.basis_json(),
        allocation(result.total),
        q(net),
        period_json.join(",")
    );
    Ok(response(payload, markdown, scope))
}

fn capacity_limit<'a, 'b>(
    row: &Json,
    kind: &str,
    periods: &'a mut [DeclaredPeriod<'_>],
    parcels: &BTreeMap<&str, usize>,
    caps: &'a mut [Option<ExactQty>],
    metal_unit: &'b str,
) -> Result<(&'a mut ExactQty, &'b str), Refusal> {
    let required: &[&str] = match kind {
        "stockpile" => &["parcel"],
        "market" => &["period"],
        "route_throughput" | "route_energy" => &["period", "route"],
        _ => return Err(declaration(format!("unknown capacity option kind {kind}"))),
    };
    if CAPACITY_TARGET_FIELDS
        .into_iter()
        .any(|key| row.get(key).is_some() != required.contains(&key))
    {
        return Err(declaration(format!(
            "{kind} capacity option requires only these target fields: {}",
            required.join(", ")
        )));
    }
    if kind == "stockpile" {
        let id = label(row, "parcel")?;
        let parcel = *parcels
            .get(id)
            .ok_or_else(|| declaration(format!("unknown stockpile parcel {id}")))?;
        let cap = caps[parcel].as_mut().ok_or_else(|| {
            declaration(format!("stockpile parcel {id} has no declared base cap"))
        })?;
        return Ok((cap, "dry_metric_tonne"));
    }
    let id = label(row, "period")?;
    let period = periods
        .iter_mut()
        .find(|period| period.id == id)
        .ok_or_else(|| declaration(format!("unknown capacity option period {id}")))?;
    if kind == "market" {
        return Ok((&mut period.market, metal_unit));
    }
    let id = label(row, "route")?;
    let route = period
        .route_ids
        .iter()
        .position(|name| *name == id)
        .ok_or_else(|| {
            declaration(format!(
                "unknown capacity option route {id} in {}",
                period.id
            ))
        })?;
    let route = &mut period.routes[route];
    if kind == "route_throughput" {
        return Ok((&mut route.dry_tonnes_cap, "dry_metric_tonne"));
    }
    let cap = route.energy_budget_kwh.as_mut().ok_or_else(|| {
        declaration(format!(
            "route {id} has no declared base energy budget in {}",
            period.id
        ))
    })?;
    Ok((cap, "kWh"))
}

struct CapacityComparison {
    markdown: String,
    json: String,
}

struct CapacityChanges {
    json: String,
    targets: String,
    limits: String,
    package: bool,
}

fn capacity_arithmetic(_: fmine_units::UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        "capacity comparison exceeds exact arithmetic range",
    )
}

fn apply_capacity_changes(
    changes: &[Json],
    periods: &mut [DeclaredPeriod<'_>],
    parcel_index: &BTreeMap<&str, usize>,
    caps: &mut [Option<ExactQty>],
    metal_unit: &str,
) -> Result<CapacityChanges, Refusal> {
    let mut seen = BTreeSet::new();
    let mut json = Vec::new();
    let mut targets = Vec::new();
    let mut limits = Vec::new();
    let package = changes.len() > 1;
    for row in changes {
        if package {
            object(row, &["kind", "period", "route", "parcel", "increase"])?;
        }
        let kind = field(row, "kind")?;
        let increase = quantity(row, "increase")?;
        if increase.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "capacity changes require a positive increase",
            ));
        }
        let (cap, unit) = capacity_limit(row, kind, periods, parcel_index, caps, metal_unit)?;
        let target = CAPACITY_TARGET_FIELDS.map(|key| row.get(key).and_then(Json::as_str));
        if !seen.insert((kind, target)) {
            return Err(declaration(format!(
                "capacity package repeats a {kind} target"
            )));
        }
        let current = *cap;
        let proposed = current.checked_add(increase).map_err(capacity_arithmetic)?;
        *cap = proposed;
        let capacity_name = match kind {
            "route_energy" => "Grinding energy",
            "route_throughput" => "Throughput",
            "market" => "Offtake",
            _ => "Stockpile",
        };
        let target_label = target
            .into_iter()
            .flatten()
            .map(markdown_cell)
            .collect::<Vec<_>>()
            .join(" / ");
        let target_json = CAPACITY_TARGET_FIELDS
            .into_iter()
            .zip(target)
            .filter_map(|(key, value)| value.map(|value| format!(",\"{key}\":{}", js(value))))
            .collect::<String>();
        targets.push(format!("{capacity_name} · {target_label}"));
        limits.push(format!("{} → {} {unit}", exact(current), exact(proposed)));
        let fields = format!(
            "\"kind\":{}{target_json},\"capacity_unit\":{},\"base_cap\":{},\"new_cap\":{}",
            js(kind),
            js(unit),
            q(current),
            q(proposed)
        );
        json.push(if package {
            format!("{{{fields}}}")
        } else {
            fields
        });
    }
    let fields = json.join(",");
    Ok(CapacityChanges {
        json: if package {
            format!("\"kind\":\"package\",\"changes\":[{fields}]")
        } else {
            fields
        },
        targets: targets.join(" + "),
        limits: limits.join(", "),
        package,
    })
}

struct EvaluatedCapacity<'a> {
    id: &'a str,
    cost: ExactQty,
    changes: CapacityChanges,
    result: Option<HorizonEvaluation<'a>>,
}

fn evaluate_capacity<'a>(
    declared_options: &'a [Json],
    horizon: Horizon<'_>,
    periods: &[DeclaredPeriod<'_>],
    parcel_index: &BTreeMap<&str, usize>,
    metal_unit: &str,
    campaign: Option<Campaign<'a>>,
) -> Result<Vec<EvaluatedCapacity<'a>>, Refusal> {
    if declared_options.is_empty() {
        return Err(declaration("capacity_options requires at least one option"));
    }
    let mut ids = BTreeSet::new();
    let mut evaluations = Vec::with_capacity(declared_options.len());
    for row in declared_options {
        let changes = if row.get("changes").is_some() {
            object(row, &["id", "present_value_cost", "changes"])?;
            let changes = rows(row, "changes", MAX_PACKAGE_CHANGES)?;
            if changes.len() < 2 {
                return Err(declaration(
                    "a capacity package requires at least two changes",
                ));
            }
            changes
        } else {
            object(
                row,
                &[
                    "id",
                    "kind",
                    "period",
                    "route",
                    "parcel",
                    "increase",
                    "present_value_cost",
                ],
            )?;
            std::slice::from_ref(row)
        };
        let id = label(row, "id")?;
        if !ids.insert(id) {
            return Err(declaration(format!("duplicate capacity option {id}")));
        }
        let cost = quantity(row, "present_value_cost")?;
        if cost.signum() < 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("capacity option {id} requires a nonnegative present-value cost"),
            ));
        }
        let mut variant = periods.to_vec();
        let mut caps = horizon.stockpile_caps_dry_tonnes.to_vec();
        let changes =
            apply_capacity_changes(changes, &mut variant, parcel_index, &mut caps, metal_unit)?;
        let models: Vec<_> = variant.iter().map(DeclaredPeriod::model).collect();
        let result = solve_declared_horizon(
            Horizon {
                periods: &models,
                stockpile_caps_dry_tonnes: &caps,
                ..horizon
            },
            &variant,
            campaign,
        )?;
        evaluations.push(EvaluatedCapacity {
            id,
            cost,
            changes,
            result,
        });
    }
    Ok(evaluations)
}

fn campaign_cells(
    result: &HorizonEvaluation<'_>,
    periods: &[DeclaredPeriod<'_>],
) -> (String, String) {
    match &result.campaign {
        Some(choice) => (
            format!(" {} |", choice.path_label()),
            format!(
                ",\"campaign\":{}",
                choice.json(periods, result.routing.discounted_contribution)
            ),
        ),
        None => (String::new(), String::new()),
    }
}

fn compare_capacity(
    report: &Report<'_>,
    periods: &[DeclaredPeriod<'_>],
    evaluations: Vec<EvaluatedCapacity<'_>>,
    base: ExactQty,
    campaign: bool,
) -> Result<CapacityComparison, Refusal> {
    let scope = if evaluations.iter().any(|option| option.changes.package) {
        PACKAGE_SCOPE
    } else {
        CAPACITY_SCOPE
    };
    let currency = report.currency;
    let (campaign_header, campaign_separator, comparison_basis) = if campaign {
        (
            " Mode schedule |",
            "---|",
            ",\"comparison_basis\":\"net_of_campaign_switch_cost\"",
        )
    } else {
        ("", "", "")
    };
    let mut markdown = format!(
        "\n## Capacity options\n\n| Option | Capacity and target | Current to proposed cap | Break-even cost ({currency}) | Quoted cost ({currency}) | Net improvement ({currency}) |{campaign_header}\n|---|---|---:|---:|---:|---:|{campaign_separator}\n"
    );
    let mut best = Vec::new();
    let mut best_net = ExactQty::from_integer(0);
    let mut options = Vec::new();
    for EvaluatedCapacity {
        id,
        cost,
        changes,
        result,
    } in evaluations
    {
        let result = result.ok_or_else(|| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "capacity option {id} made a feasible horizon infeasible after relaxing a limit"
                ),
            )
        })?;
        let value = result.net();
        let gain = value.checked_sub(base).map_err(capacity_arithmetic)?;
        if gain.signum() < 0 {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!("capacity option {id} decreased the optimum after relaxing a limit"),
            ));
        }
        let net = gain.checked_sub(cost).map_err(capacity_arithmetic)?;
        if net
            .checked_cmp(best_net)
            .map_err(capacity_arithmetic)?
            .is_gt()
        {
            best_net = net;
            best.clear();
        }
        if net == best_net && net.signum() > 0 {
            best.push(id);
        }
        let (campaign_cell, campaign_json) = campaign_cells(&result, periods);
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {} |{campaign_cell}",
            markdown_cell(id),
            changes.targets,
            changes.limits,
            exact(gain),
            exact(cost),
            exact(net)
        );
        options.push(format!(
            "{{\"id\":{},{},\"discounted_contribution\":{},\"break_even_present_value_cost\":{},\"present_value_cost\":{},\"net_present_value_improvement\":{},\"total\":{{{}}}{campaign_json}}}",
            js(id), changes.json, q(value),
            q(gain), q(cost), q(net), allocation(result.routing.total)
        ));
    }
    if best.is_empty() {
        let _ = writeln!(
            markdown,
            "\nNo option improves on the base case after its quoted cost."
        );
    } else {
        let names = best
            .iter()
            .map(|id| markdown_cell(id))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            markdown,
            "\n**Best by net value** {names} at **{} {currency}** above the base case.",
            exact(best_net)
        );
    }
    let _ = writeln!(markdown, "\n{scope}");
    if campaign {
        markdown.push_str(
            "\nBreak-even values compare contribution after discounted changeover costs.\n",
        );
    }
    let best = best.into_iter().map(js).collect::<Vec<_>>().join(",");
    let json = format!(
        "{{\"base_discounted_contribution\":{},\"best_positive_options\":[{best}],\"options\":[{}],\"scope\":{}{comparison_basis}}}",
        q(base),
        options.join(","),
        js(scope)
    );
    Ok(CapacityComparison { markdown, json })
}

#[derive(Default)]
struct QuoteRank<'a> {
    value: Option<ExactQty>,
    ids: Vec<&'a str>,
}

impl<'a> QuoteRank<'a> {
    fn consider(&mut self, id: &'a str, value: ExactQty, better: Ordering) -> Result<(), Refusal> {
        let order = self
            .value
            .map(|current| value.checked_cmp(current))
            .transpose()
            .map_err(capacity_arithmetic)?;
        if order.is_none() || order == Some(better) {
            self.value = Some(value);
            self.ids.clear();
        }
        if self.value == Some(value) {
            self.ids.push(id);
        }
        Ok(())
    }

    fn write(&self, markdown: &mut String, label: &str, currency: &str) -> String {
        if let Some(value) = self.value {
            let names = self
                .ids
                .iter()
                .map(|id| markdown_cell(id))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                markdown,
                "\n**{label}** {names} at **{} {currency}**.",
                exact(value)
            );
        }
        self.ids
            .iter()
            .map(|id| js(id))
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn render_capacity_rescue(
    report: &Report<'_>,
    periods: &[DeclaredPeriod<'_>],
    evaluations: Vec<EvaluatedCapacity<'_>>,
    campaign: bool,
) -> Result<Envelope, Refusal> {
    let Report {
        currency,
        metal,
        unit,
        evidence,
        source,
        ..
    } = *report;
    let baseline = horizon_infeasible(periods, campaign);
    let mut markdown = format!(
        "# Capacity quotes for an infeasible ore-routing horizon\n\n{} · {unit} · {currency} · 100% project basis\n\nInputs are {evidence} and uncalibrated. Source SHA-256 \x60{}\x60.\n\nThe base horizon is infeasible. Each quote is evaluated against the same commitments.\n\n| Period | Recovered-metal minimum ({unit}) | Base market cap ({unit}) |\n|---|---:|---:|\n",
        markdown_cell(metal),
        fmine_core::Digest32(source.artifact_sha256)
    );
    let mut declarations = Vec::with_capacity(periods.len());
    for period in periods {
        let _ = writeln!(
            markdown,
            "| {} | {} | {} |",
            markdown_cell(period.id),
            period.market_minimum.map_or_else(|| "—".into(), exact),
            exact(period.market)
        );
        declarations.push(format!(
            "{{\"period\":{},\"market_recovered_metal_minimum\":{},\"market_recovered_metal_cap\":{}}}",
            js(period.id), maybe(period.market_minimum), q(period.market)
        ));
    }
    let (campaign_header, campaign_separator) = if campaign {
        (" Mode schedule |", "---|")
    } else {
        ("", "")
    };
    let _ = writeln!(
        markdown,
        "\n| Option | Capacity and target | Current to proposed cap | Feasibility | Discounted contribution ({currency}) | Quoted cost ({currency}) | Contribution after quote ({currency}) |{campaign_header}\n|---|---|---:|---|---:|---:|---:|{campaign_separator}"
    );
    let mut best = QuoteRank::default();
    let mut cheapest = QuoteRank::default();
    let mut options = Vec::with_capacity(evaluations.len());
    let cell = |quantity: Option<ExactQty>| quantity.map_or_else(|| "—".into(), exact);
    for EvaluatedCapacity {
        id,
        cost,
        changes,
        result,
    } in evaluations
    {
        let (feasibility, value, after_quote, total, deliveries, mode_cell, campaign_json) =
            if let Some(result) = result {
                let value = result.net();
                let after_quote = value.checked_sub(cost).map_err(capacity_arithmetic)?;
                best.consider(id, after_quote, Ordering::Greater)?;
                cheapest.consider(id, cost, Ordering::Less)?;
                let deliveries = periods
                    .iter()
                    .zip(&result.routing.periods)
                    .map(|(period, routing)| {
                        format!(
                            "{{\"period\":{},\"recovered_metal\":{}}}",
                            js(period.id),
                            q(routing.total.recovered_metal)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let (mode_cell, campaign_json) = campaign_cells(&result, periods);
                (
                    "feasible",
                    Some(value),
                    Some(after_quote),
                    format!("{{{}}}", allocation(result.routing.total)),
                    format!("[{deliveries}]"),
                    mode_cell,
                    campaign_json,
                )
            } else {
                (
                    "infeasible",
                    None,
                    None,
                    "null".into(),
                    "null".into(),
                    if campaign {
                        " — |".into()
                    } else {
                        String::new()
                    },
                    String::new(),
                )
            };
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {feasibility} | {} | {} | {} |{mode_cell}",
            markdown_cell(id),
            changes.targets,
            changes.limits,
            cell(value),
            exact(cost),
            cell(after_quote)
        );
        options.push(format!(
            "{{\"id\":{},{},\"feasibility\":{},\"discounted_contribution\":{},\"present_value_cost\":{},\"contribution_after_quote\":{},\"break_even_present_value_cost\":null,\"net_present_value_improvement\":null,\"total\":{total},\"period_deliveries\":{deliveries}{campaign_json}}}",
            js(id), changes.json, js(feasibility), maybe(value), q(cost), maybe(after_quote)
        ));
    }
    if best.ids.is_empty() {
        markdown.push_str("\nNo submitted quote makes the horizon feasible.\n");
    }
    let cheapest = cheapest.write(&mut markdown, "Lowest quoted cost", currency);
    let best = best.write(&mut markdown, "Highest contribution after quote", currency);
    let _ = writeln!(markdown, "\n{FEASIBILITY_SCOPE}");
    let payload = format!(
        "{{\"schema\":\"{FEASIBILITY_SCHEMA}\",{},\"total\":null,\"periods\":[{}],\"capacity_comparison\":{{\"baseline_status\":\"infeasible\",\"baseline_detail\":{},\"base_discounted_contribution\":null,\"cheapest_feasible_options\":[{cheapest}],\"best_feasible_options\":[{best}],\"options\":[{}],\"scope\":{}}}}}",
        report.basis_json(),
        declarations.join(","),
        js(&baseline.what),
        options.join(","),
        js(FEASIBILITY_SCOPE)
    );
    Ok(response(payload, markdown, FEASIBILITY_SCOPE))
}

fn response(payload: String, markdown: String, scope: &'static str) -> Envelope {
    let mut envelope = Envelope::ok(VERB, payload, vec![]);
    envelope.no_claim = Some(scope);
    envelope.rendered_markdown = Some(markdown);
    envelope
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
