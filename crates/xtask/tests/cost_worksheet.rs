//! BD-061: usable exact cost worksheets through the CLI, with invented declared inputs.
//! Invented dollar amounts exercise arithmetic. The WGC case checks cite published project facts
//! and identify every extra assumption supplied by the test (E875).

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke, parse_args, run};
use fmine_core::hash::sha256;

const REGISTRY: &str = include_str!("../../../registries/metrics.toml");
const SOURCE: &str = "worksheet.toml";
const PROJECTS: &str = include_str!("../../../tests/costs/classified_projects.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn classified_project_costs_reach_aisc_aic_with_all_assumptions_and_excluded_amounts() {
    let fixture = Fixture::new(PROJECTS);
    let aisc = fixture.ok("aisc@wgc-2018");
    let aic = fixture.ok("aic@wgc-2018");
    // The 400 expansion clears both thresholds. The 200 extension's five-year ore
    // phase does not strictly exceed five years, so it remains sustaining.
    assert_eq!(string_field(&aisc.payload_json, "numerator_usd"), "1000");
    assert_eq!(string_field(&aisc.payload_json, "usd_per_oz"), "10");
    assert_eq!(string_field(&aic.payload_json, "numerator_usd"), "1400");
    assert_eq!(string_field(&aic.payload_json, "usd_per_oz"), "14");
    for result in [&aisc, &aic] {
        for expected in [
            "\"project_classifications\":[",
            "\"id\":\"plant-expansion\"",
            "\"id\":\"pit-extension\"",
            "\"classification\":\"non_sustaining\"",
            "\"classification\":\"sustaining\"",
            "\"material_uplift_percent\":\"10\"",
            "\"duration_at_least_threshold\":true",
            "\"ore_phase_strictly_above_threshold\":false",
            "project_cost.uplift_percent",
            "project_cost.duration_months",
            "project_test aisc@wgc-2018: exact classification thresholds",
        ] {
            assert!(
                result.payload_json.contains(expected),
                "{expected}: {}",
                result.payload_json
            );
        }
        assert!(
            result
                .payload_json
                .contains(&sha256(PROJECTS.as_bytes()).to_hex())
        );
        assert!(
            result
                .payload_json
                .contains(&sha256(REGISTRY.as_bytes()).to_hex())
        );
        assert!(result.to_json().contains("\"execution_authorized\":false"));
        let markdown = result.rendered_markdown.as_ref().unwrap();
        assert!(markdown.contains("Conditional project classifications"));
        assert!(markdown.contains("ore phase > 5 years"));
        assert!(markdown.contains("at month 13"));
    }
    assert!(aisc.payload_json.contains(
        "\"amount_minor\":\"4000000\",\"treatment\":\"excluded\",\"numerator_usd\":\"0\""
    ));
    assert_eq!(fixture.ok("aisc@wgc-2018").to_json(), aisc.to_json());
    fixture.write(&PROJECTS.replace("months_to_net_cash = \"13\"", "months_to_net_cash = \"12\""));
    let cash_within_window = fixture.ok("aisc@wgc-2018");
    assert_eq!(
        string_field(&cash_within_window.payload_json, "usd_per_oz"),
        "14"
    );
    assert_ne!(
        string_field(&cash_within_window.payload_json, "digest"),
        string_field(&aisc.payload_json, "digest")
    );
    fixture.write(&PROJECTS.replace("ore_phase_years = \"5\"", "ore_phase_years = \"5.0001\""));
    assert_eq!(
        string_field(&fixture.ok("aisc@wgc-2018").payload_json, "usd_per_oz"),
        "8"
    );
    fixture.write(&PROJECTS.replace(
        "months_to_net_cash = \"13\"",
        "months_to_net_cash = \"never\"",
    ));
    let never = fixture.ok("aisc@wgc-2018");
    assert_eq!(string_field(&never.payload_json, "usd_per_oz"), "10");
    assert_ne!(
        string_field(&never.payload_json, "digest"),
        string_field(&aisc.payload_json, "digest")
    );
}

#[test]
fn multiple_projects_keep_separate_contributions_and_refuse_ambiguous_coverage() {
    let extra = "\n[[project_cost]]\nid = \"second-expansion\"\ncost_kind = \"capital_expenditure\"\nusd = \"50\"\nclassification_rule = \"aisc@wgc-2018\"\nproject_kind = \"existing_operation\"\nuplift_percent = \"10\"\nmonths_to_net_cash = \"13\"\n";
    let text = format!("{PROJECTS}{extra}");
    let fixture = Fixture::new(&text);
    let aic = fixture.ok("aic@wgc-2018");
    assert_eq!(string_field(&aic.payload_json, "numerator_usd"), "1450");
    assert_eq!(string_field(&aic.payload_json, "usd_per_oz"), "29/2");
    assert_eq!(
        string_field(&fixture.ok("aisc@wgc-2018").payload_json, "usd_per_oz"),
        "10"
    );
    for replacement in [
        text.replace("second-expansion", "plant-expansion"),
        format!(
            "{PROJECTS}\n[[cost]]\nitem = \"capital_expenditure_non_sustaining\"\nusd = \"1\"\n"
        ),
        format!(
            "{PROJECTS}\n[[driver_cost]]\nitem = \"capital_expenditure_non_sustaining\"\ndriver = \"labour\"\nunit = \"hour\"\nquantity = \"1\"\nusd_per_unit = \"1\"\n"
        ),
    ] {
        fixture.write(&replacement);
        assert_eq!(
            fixture.invoke("aic@wgc-2018").refusal.unwrap().code,
            "FMINE-REF-105"
        );
    }
    fixture.write(PROJECTS);
    fs::write(
        fixture.0.join("driver.toml"),
        MINING_HISTORY
            .replace("on_site_mining_costs", "capital_expenditure_non_sustaining")
            .replace("Example Mine", "Site A"),
    )
    .unwrap();
    let mut args = Fixture::args("aic@wgc-2018");
    args.extend(["--fit-driver".into(), "driver.toml".into()]);
    let refusal = invoke(&args, &fixture.0).0.refusal.unwrap();
    assert_eq!(refusal.code, "FMINE-REF-105");
    assert!(refusal.what.contains("overlaps a direct or driver"));
}

#[test]
fn missing_project_facts_unknown_rules_and_inapplicable_assumptions_refuse() {
    let fixture = Fixture::new(PROJECTS);
    for text in [
        PROJECTS.replace("months_to_net_cash = \"13\"\n", ""),
        PROJECTS.replace(
            "months_to_net_cash = \"13\"",
            "months_to_net_cash = \"unknown\"",
        ),
        PROJECTS.replace(
            "months_to_net_cash = \"13\"",
            "months_to_net_cash = \"12.5\"",
        ),
        PROJECTS.replace("months_to_net_cash = \"13\"", "months_to_net_cash = \"-1\""),
        PROJECTS.replace(
            "project_kind = \"existing_operation\"",
            "project_kind = \"new_operation\"",
        ),
        PROJECTS.replace(
            "project_kind = \"extension\"",
            "project_kind = \"extension\"\nuplift_percent = \"10\"",
        ),
        PROJECTS.replace("ore_phase_years = \"5\"", "ore_phase_years = \"-1\""),
        PROJECTS.replace("usd = \"400\"", "usd = \"-400\""),
        PROJECTS.replace(
            "classification_rule = \"aisc@wgc-2018\"",
            "classification_rule = \"unregistered\"",
        ),
        PROJECTS.replace(
            "cost_kind = \"capital_expenditure\"",
            "cost_kind = \"unknown\"",
        ),
    ] {
        fixture.write(&text);
        let result = fixture.invoke("aisc@wgc-2018");
        assert_ne!(result.exit_code, 0, "accepted {text}");
        assert!(!result.payload_json.contains("\"usd_per_oz\""));
    }
    fixture.write(PROJECTS);
    assert_eq!(
        fixture.invoke("aisc@wgc-2013").refusal.unwrap().code,
        "FMINE-REF-105"
    );
    let baseline = fixture.ok("aisc@wgc-2018");
    fs::write(
        fixture.0.join("registries/metrics.toml"),
        REGISTRY.replace(
            "material_uplift_percent = \"10\"",
            "material_uplift_percent = \"11\"",
        ),
    )
    .unwrap();
    let changed = fixture.ok("aisc@wgc-2018");
    assert_eq!(string_field(&changed.payload_json, "usd_per_oz"), "14");
    assert_ne!(
        string_field(&changed.payload_json, "digest"),
        string_field(&baseline.payload_json, "digest")
    );
}

#[test]
fn fitted_driver_cost_reaches_registered_aisc_with_its_full_model_receipt() {
    let history = include_str!("../../../tests/costs/fitted_driver.toml");
    let fixture = Fixture::new(include_str!("../../../tests/costs/fitted_costs.toml"));
    fs::write(fixture.0.join("driver.toml"), history).unwrap();
    let mut args = Fixture::args("aisc@wgc-2018");
    args.extend(["--fit-driver".into(), "driver.toml".into()]);
    let (result, _) = invoke(&args, &fixture.0);
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    assert_eq!(string_field(&result.payload_json, "usd_per_oz"), "6");
    for expected in [
        "\"fitted_driver\":{",
        "fmine.cost-driver-result.v1",
        "\"fixed_usd\":\"150\"",
        "\"usd_per_unit\":\"9/5\"",
        "\"mean_variance_usd2\":\"2250\"",
        "\"observation_variance_usd2\":\"11250\"",
        "synthetic-register-2026",
        "\"calibrated\":false",
    ] {
        assert!(result.payload_json.contains(expected), "{expected}");
    }
    assert!(
        result
            .payload_json
            .contains(&sha256(history.as_bytes()).to_hex())
    );
    assert!(
        result
            .rendered_markdown
            .as_ref()
            .unwrap()
            .contains("Observed minus fitted")
    );
    assert_eq!(invoke(&args, &fixture.0).0.to_json(), result.to_json());
    fs::write(
        fixture.0.join("driver.toml"),
        history.replace("quantity = \"250\"", "quantity = \"300\""),
    )
    .unwrap();
    let changed = invoke(&args, &fixture.0).0;
    assert_eq!(changed.exit_code, 0, "{}", changed.to_json());
    assert_eq!(string_field(&changed.payload_json, "usd_per_oz"), "69/10");
    assert_ne!(
        string_field(&changed.payload_json, "digest"),
        string_field(&result.payload_json, "digest")
    );
    fs::write(
        fixture.0.join("driver.toml"),
        history.replace("quantity = \"250\"", "quantity = \"401\""),
    )
    .unwrap();
    assert_eq!(
        invoke(&args, &fixture.0).0.refusal.unwrap().code,
        "FMINE-REF-203"
    );
}

const MINING_HISTORY: &str = include_str!("../../../tests/costs/fitted_driver.toml");
const LABOUR_HISTORY: &str = include_str!("../../../tests/costs/fitted_labour_driver.toml");
const MULTI_COSTS: &str = include_str!("../../../tests/costs/fitted_multi_costs.toml");

fn multi_driver_fixture() -> Fixture {
    let fixture = Fixture::new(MULTI_COSTS);
    fs::write(fixture.0.join("mining.toml"), MINING_HISTORY).unwrap();
    fs::write(fixture.0.join("labour.toml"), LABOUR_HISTORY).unwrap();
    fixture
}

fn with_drivers(paths: &[&str]) -> Vec<String> {
    let mut args = Fixture::args("aisc@wgc-2018");
    for path in paths {
        args.extend(["--fit-driver".into(), (*path).into()]);
    }
    args
}

#[test]
fn complete_multi_pool_cost_plan_keeps_all_models_and_has_order_independent_reports() {
    let fixture = multi_driver_fixture();
    let args = with_drivers(&["mining.toml", "labour.toml"]);
    let (result, _) = invoke(&args, &fixture.0);
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    // Independent expected cost: mining 600 + administration 230 + community 25;
    // the separate 100 USD lease cost and by-product credit cancel.
    assert_eq!(string_field(&result.payload_json, "numerator_usd"), "855");
    assert_eq!(string_field(&result.payload_json, "usd_per_oz"), "171/20");
    assert!(result.payload_json.contains("\"fitted_drivers\":["));
    assert!(!result.payload_json.contains("\"fitted_driver\":"));
    assert_eq!(
        result
            .payload_json
            .matches("fmine.cost-driver-result.v1")
            .count(),
        2
    );
    for source in [MINING_HISTORY, LABOUR_HISTORY, MULTI_COSTS] {
        assert!(
            result
                .payload_json
                .contains(&sha256(source.as_bytes()).to_hex())
        );
    }
    for expected in [
        "synthetic-register-2026",
        "synthetic-labour-register-2029",
        "\"fixed_usd\":\"50\"",
        "\"usd_per_unit\":\"6\"",
        "\"mean_variance_usd2\":\"2250\"",
        "\"mean_variance_usd2\":\"300\"",
        "\"status\":\"not_estimated\"",
        "individual fitted variances are not summed",
        "\"calibrated\":false",
        "community_costs_current_operations",
    ] {
        assert!(result.payload_json.contains(expected), "missing {expected}");
    }
    let markdown = result.rendered_markdown.as_ref().unwrap();
    assert!(markdown.contains("Joint cost uncertainty is not estimated"));
    assert_eq!(markdown.matches("## Fitted cost driver\n").count(), 2);
    assert!(markdown.contains("on_site_general_and_administrative"));
    assert!(markdown.contains("on_site_mining_costs"));
    assert_eq!(
        invoke(&with_drivers(&["labour.toml", "mining.toml"]), &fixture.0)
            .0
            .to_json(),
        result.to_json()
    );
    let mut markdown_args = args.clone();
    markdown_args.extend(["--format".into(), "markdown".into()]);
    assert_eq!(run(&markdown_args, &fixture.0), (markdown.clone(), 0));

    // Changing only the second pool's target by five hours increases its mean by 30 USD.
    let changed_history = LABOUR_HISTORY.replacen("quantity = \"30\"", "quantity = \"35\"", 1);
    fs::write(fixture.0.join("labour.toml"), &changed_history).unwrap();
    let changed = invoke(&args, &fixture.0).0;
    assert_eq!(changed.exit_code, 0, "{}", changed.to_json());
    assert_eq!(string_field(&changed.payload_json, "usd_per_oz"), "177/20");
    assert!(
        changed
            .payload_json
            .contains(&sha256(changed_history.as_bytes()).to_hex())
    );
    assert!(
        changed
            .payload_json
            .contains(&sha256(MINING_HISTORY.as_bytes()).to_hex())
    );
    assert_ne!(
        string_field(&changed.payload_json, "digest"),
        string_field(&result.payload_json, "digest")
    );
}

#[test]
fn any_missing_duplicate_excluded_or_incompatible_card_refuses_the_complete_cost_plan() {
    let fixture = multi_driver_fixture();
    let args = with_drivers(&["mining.toml", "labour.toml"]);
    for (text, code, detail) in [
        (
            LABOUR_HISTORY.replace("on_site_general_and_administrative", "on_site_mining_costs"),
            "FMINE-REF-105",
            "on_site_mining_costs",
        ),
        (
            LABOUR_HISTORY.replace("on_site_general_and_administrative", "income_tax"),
            "FMINE-REF-105",
            "income_tax is unused",
        ),
        (
            LABOUR_HISTORY.replace("Example Mine", "Another Site"),
            "FMINE-REF-105",
            "inputs differ",
        ),
        (
            LABOUR_HISTORY.replacen("quantity = \"30\"", "quantity = \"41\"", 1),
            "FMINE-REF-203",
            "outside",
        ),
    ] {
        fs::write(fixture.0.join("labour.toml"), &text).unwrap();
        let result = invoke(&args, &fixture.0).0;
        assert_ne!(result.exit_code, 0);
        let refusal = result.refusal.unwrap();
        assert_eq!(refusal.code, code);
        assert!(refusal.what.contains(detail), "{}", refusal.what);
        assert!(!result.payload_json.contains("\"usd_per_oz\""));
        assert!(!result.payload_json.contains("\"fitted_drivers\""));
        assert_eq!(
            fs::read_to_string(fixture.0.join("labour.toml")).unwrap(),
            text
        );
    }
    let missing = invoke(&with_drivers(&["mining.toml", "missing.toml"]), &fixture.0).0;
    assert_eq!(missing.refusal.unwrap().code, "FMINE-REF-506");
    assert!(!missing.payload_json.contains("\"usd_per_oz\""));
    assert!(parse_args(&with_drivers(&["mining.toml", "mining.toml"]), &fixture.0).is_err());
    // A second file cannot evade the unique item mapping even if its pathname differs.
    fs::write(fixture.0.join("copy.toml"), MINING_HISTORY).unwrap();
    assert_eq!(
        invoke(&with_drivers(&["mining.toml", "copy.toml"]), &fixture.0)
            .0
            .refusal
            .unwrap()
            .code,
        "FMINE-REF-105"
    );
    let paths = (0..65)
        .map(|i| format!("driver-{i}.toml"))
        .collect::<Vec<_>>();
    let paths = paths.iter().map(String::as_str).collect::<Vec<_>>();
    assert!(parse_args(&with_drivers(&paths), &fixture.0).is_err());
}

fn worksheet(lines: &[(&str, &str)]) -> String {
    let mut text = String::from(
        "schema = \"fmine.cost-worksheet.v1\"\nissuer = \"invented\"\nsubject = \"Site A\"\n\
         period = \"2030FY\"\nownership = \"full\"\nbasis = \"nominal USD accrual\"\n\
         currency = \"USD\"\ncomplete = \"true\"\nounces_sold = \"100\"\n",
    );
    for (item, usd) in lines {
        text.push_str(&format!("\n[[cost]]\nitem = \"{item}\"\nusd = \"{usd}\"\n"));
    }
    text
}

struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-cost-worksheet-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("registries")).unwrap();
        fs::write(root.join("registries/metrics.toml"), REGISTRY).unwrap();
        fs::write(root.join(SOURCE), text).unwrap();
        Self(root)
    }
    fn write(&self, text: &str) {
        fs::write(self.0.join(SOURCE), text).unwrap();
    }
    fn args(definition: &str) -> Vec<String> {
        [
            "metrics",
            "cost",
            "--input",
            SOURCE,
            "--definition",
            definition,
        ]
        .map(str::to_owned)
        .into()
    }
    fn invoke(&self, definition: &str) -> Envelope {
        let (envelope, format) = invoke(&Self::args(definition), &self.0);
        assert_eq!(format, OutputFormat::Json);
        envelope
    }
    fn ok(&self, definition: &str) -> Envelope {
        let result = self.invoke(definition);
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        assert!(result.refusal.is_none());
        result
    }
}

fn string_field<'a>(json: &'a str, field: &str) -> &'a str {
    json.split_once(&format!("\"{field}\":\""))
        .unwrap_or_else(|| panic!("missing {field}: {json}"))
        .1
        .split('"')
        .next()
        .unwrap()
}

#[test]
fn wgc_2018_splits_sustaining_costs_growth_and_leases_and_nets_distinct_credits() {
    // E875, WGC 2018 p.1 rows (k), (s), (u)–(x), (cc) and FAQ 19.
    let text = worksheet(&[
        ("on_site_mining_costs", "1000"),
        ("lease_payments", "100"),
        ("non_sustaining_leases", "300"),
        ("growth_and_development_costs_not_current_operations", "200"),
        ("community_costs_not_current_operations", "40"),
        ("permitting_costs_not_current_operations", "50"),
        ("reclamation_costs_not_current_operations", "60"),
        ("by_product_credits", "25"),
        ("co_product_credits", "75"),
    ]);
    let f = Fixture::new(&text);
    let aisc = f.ok("aisc@wgc-2018");
    let aic = f.ok("aic@wgc-2018");
    assert_eq!(string_field(&aisc.payload_json, "numerator_usd"), "1000");
    assert_eq!(string_field(&aic.payload_json, "numerator_usd"), "1650");
    assert_eq!(string_field(&aic.payload_json, "usd_per_oz"), "33/2");
    for item in [
        "non_sustaining_leases",
        "growth_and_development_costs_not_current_operations",
        "community_costs_not_current_operations",
        "permitting_costs_not_current_operations",
        "reclamation_costs_not_current_operations",
    ] {
        assert!(aisc.payload_json.contains(&format!("\"item\":\"{item}\"")));
        assert!(aic.payload_json.contains(&format!("\"item\":\"{item}\"")));
    }
    assert!(aisc.payload_json.contains(
        "\"amount_minor\":\"3000000\",\"treatment\":\"excluded\",\"numerator_usd\":\"0\""
    ));
    let changed = text.replace("usd = \"300\"", "usd = \"300.01\"");
    f.write(&changed);
    let new_aisc = f.ok("aisc@wgc-2018");
    assert_eq!(
        string_field(&new_aisc.payload_json, "numerator_usd"),
        "1000"
    );
    assert_ne!(
        string_field(&new_aisc.payload_json, "digest"),
        string_field(&aisc.payload_json, "digest")
    );
    assert_eq!(
        string_field(&f.ok("aic@wgc-2018").payload_json, "numerator_usd"),
        "165001/100"
    );
}

#[test]
fn wgc_case_studies_preserve_the_published_anchors_and_declare_missing_inputs() {
    // E875 pp.10–11. All prices and dates are invented.
    let baseline = worksheet(&[("on_site_mining_costs", "800")]);
    let f = Fixture::new(&baseline);
    let project = |id: &str, cost_kind: &str, facts: &str| {
        format!(
            "\n[[project_cost]]\nid = \"{id}\"\ncost_kind = \"{cost_kind}\"\nusd = \"200\"\nclassification_rule = \"aisc@wgc-2018\"\n{facts}\n"
        )
    };
    // Publication gives the uplift for A, B, D and E. Cash timing is supplied by the test.
    // D uses the 100 koz prior plan baseline and 150 koz expanded plan, a 50% uplift from plan.
    // F is conditional on materiality. The rows straddle that threshold. I's expansion gives
    // 5 and 8 Mtpa of plant capacity. The test assumes 60% uplift in production to run FAQ 7.
    for (id, uplift, expected) in [
        ("A", "15", "8"),
        ("B", "10", "8"),
        ("D", "50", "8"),
        ("D-current-production-control", "-25", "10"),
        ("E", "25", "8"),
        ("E-like-for-like-control", "0", "10"),
        ("F-qualifying-premise", "10", "8"),
        ("F-below-materiality", "9.999", "10"),
        ("I-expansion", "60", "8"),
    ] {
        f.write(&format!(
            "{baseline}{}",
            project(id, "capital_expenditure", &format!(
                "project_kind = \"existing_operation\"\nuplift_percent = \"{uplift}\"\nmonths_to_net_cash = \"13\""
            )),
        ));
        assert_eq!(
            string_field(&f.ok("aisc@wgc-2018").payload_json, "usd_per_oz"),
            expected,
            "{id}"
        );
        assert_eq!(
            string_field(&f.ok("aic@wgc-2018").payload_json, "usd_per_oz"),
            "10",
            "{id}"
        );
    }
    // Mine C lacks exact duration and ore phase, so exercise its printed decision boundary.
    // Mine H gives 18 months of development and seven years of ore production.
    for (id, duration, years, expected) in [
        ("C-at-boundary", 12, 5, "10"),
        ("C-above-boundary", 12, 6, "8"),
        ("H", 18, 7, "8"),
    ] {
        f.write(&format!(
            "{baseline}{}",
            project(id, "capitalised_stripping_and_underground_development", &format!(
                "project_kind = \"extension\"\nduration_months = \"{duration}\"\nore_phase_years = \"{years}\""
            )),
        ));
        assert_eq!(
            string_field(&f.ok("aisc@wgc-2018").payload_json, "usd_per_oz"),
            expected,
            "{id}"
        );
        assert_eq!(
            string_field(&f.ok("aic@wgc-2018").payload_json, "usd_per_oz"),
            "10",
            "{id}"
        );
    }
    // I's initial 5 Mtpa plant needs no claimed growth percentage or payback date.
    let initial = "project_kind = \"new_operation\"";
    let text = format!(
        "{baseline}{}{}",
        project("I-initial", "capital_expenditure", initial),
        project("I-lease", "leases", initial)
    );
    f.write(&text);
    let aisc = f.ok("aisc@wgc-2018");
    let aic = f.ok("aic@wgc-2018");
    assert_eq!(string_field(&aisc.payload_json, "usd_per_oz"), "8");
    assert_eq!(string_field(&aic.payload_json, "usd_per_oz"), "12");
    assert!(
        aic.payload_json
            .contains("\"thresholds\":{},\"conditions\":{},\"classification\":\"non_sustaining\"")
    );
    assert!(
        aic.payload_json
            .contains("\"item\":\"non_sustaining_leases\"")
    );
    assert!(
        aic.rendered_markdown
            .unwrap()
            .contains("initial development at a new operation")
    );
    f.write(&format!("{text}months_to_net_cash = \"13\"\n"));
    let refused = f.invoke("aic@wgc-2018");
    assert_ne!(refused.exit_code, 0);
    assert!(
        refused
            .refusal
            .unwrap()
            .what
            .contains("new_operation without threshold fields")
    );
}

#[test]
fn aic_command_emits_exact_complete_receipts_and_deterministic_markdown() {
    let text = worksheet(&[
        ("on_site_mining_costs", "800.01"),
        ("capital_expenditure_non_sustaining", "400"),
        ("by_product_credits", "200"),
        ("income_tax", "999"),
    ]);
    let fixture = Fixture::new(&text);
    let result = fixture.ok("aic@wgc-2018");
    assert_eq!(
        string_field(&result.payload_json, "schema"),
        "fmine.cost-worksheet.v1"
    );
    assert_eq!(
        string_field(&result.payload_json, "numerator_usd"),
        "100001/100"
    );
    assert_eq!(
        string_field(&result.payload_json, "usd_per_oz"),
        "100001/10000"
    );
    assert_eq!(string_field(&result.payload_json, "ounces_sold"), "100");
    for expected in [
        "\"declared_complete\":true",
        "\"treatment\":\"excluded\"",
        "\"inputs\":[",
        "\"claim\":{",
    ] {
        assert!(
            result.payload_json.contains(expected),
            "missing {expected}: {}",
            result.payload_json
        );
    }
    assert!(
        result
            .payload_json
            .contains(&sha256(text.as_bytes()).to_hex())
    );
    assert!(
        result
            .payload_json
            .contains(&sha256(REGISTRY.as_bytes()).to_hex())
    );
    assert!(result.to_json().contains("\"execution_authorized\":false"));
    assert!(result.no_claim.unwrap().contains("declared complete"));
    assert_eq!(fixture.ok("aic@wgc-2018").to_json(), result.to_json());
    let mut args = Fixture::args("aic@wgc-2018");
    args.extend(["--format".into(), "markdown".into()]);
    let (markdown, exit) = run(&args, &fixture.0);
    assert_eq!(exit, 0);
    assert_eq!(markdown, result.rendered_markdown.unwrap());
    assert!(markdown.contains("capital_expenditure_non_sustaining"));
    assert!(markdown.contains("100001/10000"));
}

#[test]
fn selected_versions_and_explicit_allocation_compute_distinct_useful_costs() {
    let fixture = Fixture::new(&worksheet(&[
        ("on_site_mining_costs", "800"),
        ("lease_payments", "100"),
    ]));
    for (definition, expected) in [("aisc@wgc-2013", "8"), ("aisc@wgc-2018", "9")] {
        let result = fixture.ok(definition);
        assert_eq!(string_field(&result.payload_json, "usd_per_oz"), expected);
    }
    let allocated = worksheet(&[
        ("on_site_mining_costs", "1000"),
        ("by_product_credits", "300"),
    ])
    .replace(
        "ounces_sold = \"100\"",
        "ounces_sold = \"100\"\ngold_share = \"3/4\"",
    );
    fixture.write(&allocated);
    let result = fixture.ok("aisc.co_product@wgc-2018");
    assert_eq!(string_field(&result.payload_json, "usd_per_oz"), "15/2");
    assert_eq!(string_field(&result.payload_json, "numerator_usd"), "750");
    assert_eq!(string_field(&result.payload_json, "gold_share"), "3/4");
    fixture.write(&worksheet(&[
        ("on_site_mining_costs", "800"),
        ("by_product_credits", "1000"),
    ]));
    assert_eq!(
        string_field(&fixture.ok("aisc@wgc-2018").payload_json, "usd_per_oz"),
        "-2"
    );
}

#[test]
fn inherited_cost_adjustment_changes_the_signed_cli_result_and_bound_receipt() {
    let fixture = Fixture::new(&worksheet(&[
        ("on_site_mining_costs", "1000"),
        ("royalties_and_production_taxes", "100"),
    ]));
    // Invented versioned definitions: the original 100 USD cost becomes a separately declared
    // 100 USD rebate. A second inheritance path must not restore the ancestor's cost treatment.
    let declarations = r#"
[[definition]]
id = "rebate@invented"
family = "all_in_cost"
unit = "USD/oz"
formula = "invented royalty rebate control"
source = "invented arithmetic, no issuer claim"
evidence = "E1"
extends = "aisc@wgc-2018"
[[line]]
definition = "rebate@invented"
item = "royalties_and_production_taxes"
letter = "rebate"
kind = "credit"
[[definition]]
id = "unchanged@invented"
family = "all_in_cost"
unit = "USD/oz"
formula = "invented unchanged branch"
source = "invented arithmetic, no issuer claim"
evidence = "E1"
extends = "aisc@wgc-2018"
[[definition]]
id = "combined@invented"
family = "all_in_cost"
unit = "USD/oz"
formula = "invented combined branches"
source = "invented arithmetic, no issuer claim"
evidence = "E1"
extends = "rebate@invented unchanged@invented"
"#;
    let registry = format!("{REGISTRY}{declarations}");
    fs::write(fixture.0.join("registries/metrics.toml"), &registry).unwrap();
    let original = fixture.ok("aisc@wgc-2018");
    assert_eq!(string_field(&original.payload_json, "usd_per_oz"), "11");
    let adjusted = fixture.ok("combined@invented");
    assert_eq!(string_field(&adjusted.payload_json, "numerator_usd"), "900");
    assert_eq!(string_field(&adjusted.payload_json, "usd_per_oz"), "9");
    assert!(adjusted.payload_json.contains("\"treatment\":\"credit\""));
    assert!(adjusted.payload_json.contains("\"numerator_usd\":\"-100\""));
    assert!(
        adjusted
            .payload_json
            .contains(&sha256(registry.as_bytes()).to_hex())
    );
    assert_eq!(
        fixture.ok("combined@invented").to_json(),
        adjusted.to_json()
    );

    let reordered = format!(
        "{REGISTRY}{}",
        declarations.replace(
            "rebate@invented unchanged@invented",
            "unchanged@invented rebate@invented"
        )
    );
    fs::write(fixture.0.join("registries/metrics.toml"), reordered).unwrap();
    assert_eq!(
        string_field(&fixture.ok("combined@invented").payload_json, "usd_per_oz"),
        "9"
    );

    let changed = format!(
        "{REGISTRY}{}",
        declarations.replace("kind = \"credit\"", "kind = \"cost\"")
    );
    fs::write(fixture.0.join("registries/metrics.toml"), changed).unwrap();
    let restored = fixture.ok("combined@invented");
    assert_eq!(string_field(&restored.payload_json, "usd_per_oz"), "11");
    assert_ne!(
        string_field(&adjusted.payload_json, "definition_tag"),
        string_field(&restored.payload_json, "definition_tag")
    );
    assert_ne!(
        string_field(&adjusted.payload_json, "digest"),
        string_field(&restored.payload_json, "digest")
    );

    let ambiguous = format!(
        "{registry}\n[[line]]\ndefinition = \"unchanged@invented\"\n\
         item = \"royalties_and_production_taxes\"\nletter = \"cost\"\nkind = \"cost\"\n"
    );
    fs::write(fixture.0.join("registries/metrics.toml"), ambiguous).unwrap();
    let refused = fixture.invoke("combined@invented");
    assert_ne!(refused.exit_code, 0);
    assert!(
        refused
            .refusal
            .unwrap()
            .what
            .contains("ambiguous inherited line")
    );
    assert!(!refused.payload_json.contains("\"usd_per_oz\""));
}

#[test]
fn pennies_and_actual_source_or_registry_bytes_change_the_receipt_identity() {
    let original = worksheet(&[("on_site_mining_costs", "800.01")]);
    let fixture = Fixture::new(&original);
    let before = fixture.ok("aisc@wgc-2018");
    fixture.write(&original.replace("800.01", "800.02"));
    let changed = fixture.ok("aisc@wgc-2018");
    assert_eq!(
        string_field(&before.payload_json, "usd_per_oz"),
        "80001/10000"
    );
    assert_eq!(
        string_field(&changed.payload_json, "usd_per_oz"),
        "40001/5000"
    );
    assert_ne!(
        string_field(&before.payload_json, "digest"),
        string_field(&changed.payload_json, "digest")
    );
    fixture.write(&format!(
        "{original}\n# Same values, different declared source bytes.\n"
    ));
    let changed_source = fixture.ok("aisc@wgc-2018");
    assert_eq!(
        string_field(&before.payload_json, "usd_per_oz"),
        string_field(&changed_source.payload_json, "usd_per_oz")
    );
    assert_ne!(
        string_field(&before.payload_json, "digest"),
        string_field(&changed_source.payload_json, "digest")
    );
    fs::write(
        fixture.0.join("registries/metrics.toml"),
        format!("{REGISTRY}\n# Different retained rule-file bytes.\n"),
    )
    .unwrap();
    let changed_rules = fixture.ok("aisc@wgc-2018");
    assert_eq!(
        string_field(&changed_source.payload_json, "usd_per_oz"),
        string_field(&changed_rules.payload_json, "usd_per_oz")
    );
    assert_ne!(
        string_field(&changed_source.payload_json, "digest"),
        string_field(&changed_rules.payload_json, "digest")
    );
}

#[test]
fn stated_calendar_and_attributable_basis_do_not_rescale_supplied_amounts() {
    for period in ["2030Q2", "2030H2", "2030FY"] {
        let text = worksheet(&[("on_site_mining_costs", "1000")])
            .replace("2030FY", period)
            .replace(
                "ownership = \"full\"",
                "ownership = \"attributable\"\nownership_share = \"3/5\"",
            );
        let fixture = Fixture::new(&text);
        assert_eq!(
            string_field(&fixture.ok("aisc@wgc-2018").payload_json, "usd_per_oz"),
            "10"
        );
    }
}

#[test]
fn missing_or_ambiguous_cost_inputs_refuse_without_a_result() {
    let original = worksheet(&[("on_site_mining_costs", "800")]);
    let fixture = Fixture::new(&original);
    let malformed = [
        original.replace("complete = \"true\"\n", ""),
        original.replace("complete = \"true\"", "complete = \"false\""),
        original.replace(
            "complete = \"true\"",
            "complete = \"true\"\nverified = \"true\"",
        ),
        original.replace("ounces_sold = \"100\"", "ounces_sold = \"0\""),
        original.replace("usd = \"800\"", "usd = \"1.00001\""),
        original.replace("currency = \"USD\"", "currency = \"CAD\""),
        original.replace("ownership = \"full\"", "ownership = \"attributable\""),
        original.replace("on_site_mining_costs", "unregistered_project_cost"),
        format!("{original}\n[[cost]]\nitem = \"on_site_mining_costs\"\nusd = \"800\"\n"),
    ];
    for text in malformed {
        fixture.write(&text);
        let result = fixture.invoke("aisc@wgc-2018");
        assert_ne!(result.exit_code, 0, "accepted {text}: {}", result.to_json());
        assert!(!result.payload_json.contains("\"usd_per_oz\""));
        assert_eq!(fs::read_to_string(fixture.0.join(SOURCE)).unwrap(), text);
    }
    fixture.write(&original);
    let mut args = Fixture::args("aisc@wgc-2018");
    args.extend(["--definition".into(), "aic@wgc-2018".into()]);
    assert!(parse_args(&args, &fixture.0).is_err());
    args = Fixture::args("aisc@wgc-2018");
    args.extend(["--value".into(), "123".into()]);
    assert!(parse_args(&args, &fixture.0).is_err());
    fs::rename(
        fixture.0.join(SOURCE),
        fixture.0.join("retained-worksheet.toml"),
    )
    .unwrap();
    let missing = fixture.invoke("aisc@wgc-2018");
    assert_ne!(missing.exit_code, 0);
    assert_eq!(missing.refusal.unwrap().code, "FMINE-REF-506");
}

#[test]
fn serialized_receipts_expose_the_colour_and_exact_uu_metadata_their_digest_binds() {
    use fmine_claim::{Claim, UuTerm};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EvidenceClass, color::ValidityDomain};
    use fmine_metrics::definitions::{claims, worksheet as declared};
    use fmine_registry::metrics::MetricRegistry;

    let text = worksheet(&[("on_site_mining_costs", "800")]);
    let parsed = declared::evaluate(
        &text,
        SourceAnchor::new(SOURCE, sha256(text.as_bytes()).0, "declared worksheet"),
        REGISTRY,
        SourceAnchor::new(
            "metrics.toml",
            sha256(REGISTRY.as_bytes()).0,
            "registered definitions",
        ),
        "aisc@wgc-2018",
    )
    .unwrap();
    let mut inputs = parsed.inputs().to_vec();
    let index = inputs
        .iter()
        .position(|c| matches!(c.value.value, claims::CostValue::Line { .. }))
        .unwrap();
    let input = &inputs[index];
    let derived = Claim::new(
        &input.metric_key,
        input.value.clone(),
        input.value_tag(),
        input.representation.clone(),
        input.support,
        input.additivity,
        EvidenceClass::Derived,
        input.epistemic,
        input.state,
        input.confidentiality.clone(),
        input.uu_terms.clone(),
        input.parents.clone(),
        input.anchors.clone(),
    );
    let regime = ValidityDomain::unconstrained().with("cost_basis", 1.0, 2.0);
    inputs[index] = derived.with_regime(regime.clone(), "fixture:first");
    inputs[index].uu_terms.push(UuTerm {
        scenario_id: "cost-omission".into(),
        description: "Unmapped account".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Ledger reconciliation".into(),
    });
    let reg = MetricRegistry::parse(REGISTRY).unwrap();
    let first = claims::evaluate(&reg, &inputs).unwrap();
    let id = inputs[index].claim_id;
    let body = inputs[index].canonical_body();
    inputs[index] = inputs[index].clone().with_regime(regime, "fixture:second");
    let second = claims::evaluate(&reg, &inputs).unwrap();
    assert_eq!(inputs[index].claim_id, id);
    assert_eq!(inputs[index].canonical_body(), body);
    assert_eq!(first.summary(), second.summary());
    assert_ne!(first.digest(), second.digest());
    assert!(first.to_json().contains("\"dataset\":\"fixture:first\""));
    assert!(second.to_json().contains("\"dataset\":\"fixture:second\""));
    assert!(
        second
            .to_json()
            .contains("\"lo_bits\":\"3ff0000000000000\"")
    );
    assert!(
        second
            .to_json()
            .contains("\"hi_bits\":\"4000000000000000\"")
    );
    assert!(
        second
            .to_json()
            .contains("\"downside_bits\":\"3ff0000000000000\"")
    );
    inputs[index].uu_terms[0].downside_exposure_usd_m = 1.0_f64.next_up();
    let changed_uu = claims::evaluate(&reg, &inputs).unwrap();
    assert_ne!(second.digest(), changed_uu.digest());
    assert!(
        changed_uu
            .to_json()
            .contains("\"downside_bits\":\"3ff0000000000001\"")
    );
}
