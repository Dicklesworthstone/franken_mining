//! Exact declared cost worksheets over the workspace's registered AISC/AIC definitions (BD-061).
//! Determinism: Exact

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::Refusal;
use fmine_metrics::definitions::{claims, driver, worksheet};

use crate::{Envelope, NextAction, worksheet_input};

const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MULTI_DRIVER_NO_CLAIM: &str = "Conditional exact cost plan from separately fitted cost-pool cards and declared costs and quantities. Source data, classification, completeness, timing and accounting references are not independently authenticated. Each fitted mean crosses its own declared monetary boundary once. Joint cost uncertainty is not estimated because error dependence between pools is undeclared; individual variances are not summed. No calibrated forecast, independence assumption, source audit, prospective registration or operating authorization is supplied.";
pub const USAGE: &str = "metrics cost --input <worksheet.toml> --definition <registered-id> [--fit-driver <history.toml> ...]";
pub const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.cost-worksheet.v1","input":"Regular UTF-8 TOML, at most 4 MiB; actual worksheet and workspace metric registry bytes are SHA-256 bound","fields":["schema","issuer","subject","period","ownership","basis","currency","complete"],"sold_quantity":"Exactly one of ounces_sold (gold) or pounds_sold (copper); actual sold units, never thousands, tonnes or produced units","optional_fields":["ownership_share","gold_share","copper_share"],"cost_rows":["item","usd"],"uu_rows":["id","description","trigger","downside_usd_m"],"values":"Quoted strings; complete must be true; period YYYYFY/YYYYQn/YYYYHn; full or attributable ownership; USD amounts at most four fractional digits; quantities and shares exact decimals or n/d","#,
    r#""definition":"Explicit registered gold AISC/AIC or copper C1/AISC ID; inherited lines, exclusions, deductions and successor retained; co-product allocations name the same metal as the denominator","output":"Exact numerator, gold ounces or copper pounds sold, USD/oz or USD/lb, signed contribution per input and complete canonical Claim receipts","evidence":"Assumed, Internal, uncalibrated; completeness is a declared assumption","example":"tests/costs/declared_aisc.toml","copper_example":"tests/costs/declared_copper.toml","#,
    r#""driver_rows":{"required":["item","driver","unit","quantity","usd_per_unit"],"optional":["fixed_usd"]},"driver_units":{"tonnes_mined":["dry_t","wet_t"],"tonnes_milled":["dry_t","wet_t"],"haulage":["dry_t_km","wet_t_km"],"electricity":["kWh"],"reagent":["kg"],"labour":["hour"]},"driver_arithmetic":"Nonnegative exact quantity times USD rate plus nonnegative fixed_usd (defaults explicitly to zero); result must fit USD 0.0001 units exactly; no rounding, implicit conversion or duplicate direct/driver item","driver_example":"tests/costs/planned_drivers.toml","#,
    r#""priced_sale_rows":{"required":["metal","unit","quantity_sold","usd_per_unit"],"optional":["royalty_fraction","by_product_credit_fraction"],"rules":"Gold worksheets. One declared sale per gold, silver (troy oz) or copper (lb). At least one gross-sales fraction in [0,1]. Gold quantity equals denominator ounces and cannot supply a by-product credit. Fractions use the same price times quantity and charges must fit USD 0.0001 exactly. Generated items cannot overlap direct or fitted cost pools. Gross sales and treatment are conditional on supplied terms.","example":"tests/costs/priced_sales.toml"},"#,
    r#""project_cost_rows":{"required":["id","cost_kind","usd","classification_rule","project_kind"],"cost_kinds":["capital_expenditure","exploration_and_study","capital_exploration","capitalised_stripping_and_underground_development"],"existing_operation":["uplift_percent","months_to_net_cash"],"extension":["duration_months","ore_phase_years"],"timing":"months_to_net_cash is nonnegative whole months or explicit never; unknown and missing facts refuse","rule":"Selected cost definition must inherit the explicitly named registered classification_rule; exact thresholds and both conditions are shown. Existing operation uses its declared largest annual/LOM production, NPV or reserve uplift and first incremental net cash. Extension uses duration and ore phase. Both conditions are required for non-sustaining classification.","coverage":"Unique project IDs retain separate contributions. An item generated from a project cannot also appear as a direct, declared-driver or fitted-driver line. No account mapping or project facts are independently verified.","output":"Source-bound project assumptions, registered thresholds, condition results, selected sustaining/non-sustaining item and metric treatment; exclusions remain visible","example":"tests/costs/classified_projects.toml"},"#,
    r#""fit_driver":{"option":"--fit-driver <history.toml>; repeat for each explicit cost pool","schema":"fmine.cost-driver.v1","limit":"At most 64 cards; each at most 1 MiB and 3..128 observation rows","fields":["schema","issuer","subject","basis","ownership","currency","item","driver","unit","period","quantity","domain_min","domain_max","valid_from","valid_through","information_cut","error_basis","monetary_rounding"],"optional_fields":["ownership_share"],"observation_fields":["period","quantity","usd","available","accounting_certificate"],"error_basis":"iid_homoskedastic_cost_errors_fixed_driver","monetary_rounding":"nearest_ties_even_0_0001_usd","#,
    r#""rules":"Each history's explicit item maps to exactly one included cost pool. Duplicate files, duplicate pools, direct/declared-driver overlaps and fitted items excluded by the selected definition refuse. Disjoint equally long training periods on the same issuer, site, ownership and accounting basis. Training availability must precede the target; constant drivers refuse. Target is inside declared driver/calendar validity, and driver domain lies within observed extrema. Signed OLS coefficients and residuals remain signed. Each fit rounds its exact mean once; all predictions use the same exact cost-definition owner.","output":"Original Claim populations, source hashes, accounting references, fixed costs, variable rates, residuals, coefficient covariance, fitted-mean and new-observation variance retained per pool. Single-card fitted_driver is unchanged; multiple cards use fitted_drivers in item order. Joint uncertainty is not estimated: cross-pool error dependence is undeclared and individual variances are not summed. No calibrated forecast or independence is inferred.","history_example":"tests/costs/fitted_driver.toml","cost_example":"tests/costs/fitted_costs.toml","multi_pool_example":"tests/costs/fitted_multi_costs.toml","additional_history_example":"tests/costs/fitted_labour_driver.toml"}}"#,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
    pub definition: String,
    pub fit_drivers: Vec<PathBuf>,
}

/// Common output/workspace options are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("metrics")
        || args.get(1).map(String::as_str) != Some("cost")
    {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut fit_drivers = Vec::new();
    let mut paths = BTreeSet::new();
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if !matches!(flag, "--input" | "--definition" | "--fit-driver") {
            return Err(format!("metrics cost: unsupported option {flag}; {USAGE}"));
        }
        let value = args
            .get(i + 1)
            .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
            .ok_or_else(|| format!("metrics cost: missing value for {flag}"))?;
        if flag == "--fit-driver" {
            let path = PathBuf::from(value);
            if !paths.insert(path.clone()) {
                return Err(format!(
                    "metrics cost: duplicate fitted-driver path {value}"
                ));
            }
            if fit_drivers.len() == worksheet::MAX_FITTED_DRIVERS {
                return Err(format!(
                    "metrics cost: at most {} fitted cost-pool cards are supported",
                    worksheet::MAX_FITTED_DRIVERS
                ));
            }
            fit_drivers.push(path);
        } else if fields.insert(flag, value).is_some() {
            return Err(format!("metrics cost: duplicate {flag}"));
        }
        i += 2;
    }
    Ok(Command {
        input: PathBuf::from(
            fields
                .get("--input")
                .ok_or("metrics cost requires --input")?
                .as_str(),
        ),
        definition: (*fields
            .get("--definition")
            .ok_or("metrics cost requires --definition")?)
        .clone(),
        fit_drivers,
    })
}

fn read(path: &Path) -> Result<(String, SourceAnchor), Refusal> {
    worksheet_input::read(path, "cost worksheet", MAX_BYTES)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = read(&root.join(&command.input))?;
        let (metrics, registry) = read(&root.join("registries/metrics.toml"))?;
        let mut fitted = command
            .fit_drivers
            .iter()
            .map(|path| {
                let (text, source) =
                    worksheet_input::read(&root.join(path), "cost driver", 1024 * 1024)?;
                driver::worksheet::evaluate(&text, source)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // A cost plan is a population of named pools; argument order is not a modelling input.
        fitted.sort_by(|a, b| a.summary().item.cmp(&b.summary().item));
        let receipt = if fitted.is_empty() {
            worksheet::evaluate(
                &text,
                source.clone(),
                &metrics,
                registry.clone(),
                &command.definition,
            )?
        } else {
            worksheet::evaluate_with_fitted_drivers(
                &text,
                source.clone(),
                &metrics,
                registry.clone(),
                &command.definition,
                &fitted,
            )?
        };
        let model_json = match fitted.as_slice() {
            [] => String::new(),
            [fit] => format!(",\"fitted_driver\":{}", fit.to_json()),
            _ => format!(
                ",\"fitted_drivers\":[{}],\"joint_uncertainty\":{{\"status\":\"not_estimated\",\"reason\":\"Cross-pool error dependence is undeclared; individual fitted variances are not summed\"}}",
                fitted
                    .iter()
                    .map(driver::DriverReceipt::to_json)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        };
        let payload = format!(
            "{{\"schema\":\"{}\",\"mode\":\"declared_cost_worksheet\",\"source\":{},\"registry\":{},\"receipt\":{}{model_json}}}",
            worksheet::SCHEMA,
            worksheet_input::source_json(&source, text.len()),
            worksheet_input::source_json(&registry, metrics.len()),
            receipt.to_json()
        );
        let mut envelope = Envelope::ok("metrics cost", payload, Vec::new());
        let no_claim = match fitted.len() {
            0 => claims::NO_CLAIM,
            1 => driver::NO_CLAIM,
            _ => MULTI_DRIVER_NO_CLAIM,
        };
        envelope.no_claim = Some(no_claim);
        envelope.warnings.push("Declared costs, classification and completeness are Assumed, Internal and uncalibrated.".into());
        if !receipt.summary().project_classifications.is_empty() {
            envelope.warnings.push("Project classification is conditional on declared facts and the selected registered rule; account classification is not independently established.".into());
        }
        let mut markdown = receipt.to_markdown();
        if !fitted.is_empty() {
            envelope.warnings.push("Cost-driver variances are conditional OLS estimates; fitted coefficients and forecast costs are uncalibrated.".into());
            markdown = markdown.replace(claims::NO_CLAIM, no_claim);
        }
        if fitted.len() > 1 {
            envelope.warnings.push("Joint cost uncertainty is not estimated: cross-pool error dependence is undeclared, and individual fitted variances are not summed.".into());
            markdown.push_str("\n## Fitted cost-pool population\n\nEach history's explicit item maps to exactly one included cost pool. The exact cost receipt sums the separately rounded point predictions and retains every source, fit and prediction. Joint cost uncertainty is not estimated: cross-pool error dependence is undeclared, and individual fitted variances are not summed.\n\n");
        }
        for fit in fitted {
            markdown.push_str(&fit.to_markdown());
        }
        envelope.rendered_markdown = Some(markdown);
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "metrics cost",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the cost-worksheet input schema and selected definition requirements",
            )],
        )
    })
}
