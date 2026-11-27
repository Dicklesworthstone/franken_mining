//! Real CLI scheduling with an independent exact Fraction and exhaustive-search oracle.
#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;

use fmine_core::{
    cert::InputDigest,
    hash::sha256,
    plan_cert::{PlanCalibrationStatus, PlanCertificate, PlanColor, PlanHorizon},
};
use fmine_io::{
    forecast::decode_hex_field,
    omf::json::{Json, parse},
};
use fmine_units::ExactQty as Rat;

fn binary() -> PathBuf {
    std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable = std::env::current_exe().unwrap();
            executable
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == "deps" || n == "build"))
                .and_then(std::path::Path::parent)
                .unwrap()
                .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        })
}

#[test]
fn split_candidates_and_global_bounds_match_independent_enumeration() {
    let output = Command::new("python3")
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/plan_schedule_oracle.py"))
        .arg(binary())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn ratio(value: &Json) -> Rat {
    let integer = |field| value.get(field).unwrap().as_str().unwrap().parse().unwrap();
    Rat::new(integer("numerator"), integer("denominator")).unwrap()
}

fn lane(root: &std::path::Path, input: &str, format: &str) -> std::process::Output {
    Command::new(binary())
        .args([
            "plan",
            "cutoff",
            "--input",
            input,
            "--format",
            format,
            "--workspace",
        ])
        .arg(root)
        .output()
        .unwrap()
}

fn lane_json(output: &std::process::Output) -> Json {
    let report = parse(&output.stdout).unwrap();
    assert_eq!(report.get("verb").unwrap().as_str(), Some("plan cutoff"));
    assert_eq!(
        report.get("exit_code").unwrap().i64(),
        output.status.code().map(i64::from)
    );
    assert!(
        report
            .get("flags")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .all(|(_, value)| { value.as_bool() == Some(false) })
    );
    report
}

fn blend(root: &std::path::Path, input: &std::path::Path, format: &str) -> std::process::Output {
    Command::new(binary())
        .args(["plan", "blend", "--input"])
        .arg(input)
        .args(["--format", format, "--workspace"])
        .arg(root)
        .output()
        .unwrap()
}

fn blend_envelope(output: &std::process::Output) -> Json {
    let report = parse(&output.stdout).unwrap();
    assert_eq!(report.get("verb").unwrap().as_str(), Some("plan blend"));
    assert_eq!(
        report.get("exit_code").unwrap().i64(),
        output.status.code().map(i64::from)
    );
    assert!(
        report
            .get("flags")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .all(|(_, flag)| flag.as_bool() == Some(false))
    );
    report
}

fn expect(row: &Json, field: &str, value: &str) {
    assert_eq!(row.get(field).unwrap().as_str(), Some(value), "{field}");
}

#[test]
fn plan_blend_routes_shared_inventory_and_replays_route_caps() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/route_blend.json";
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let run = |input, format| blend(&root, input, format);
    let output = run(input.as_ref(), "json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report = blend_envelope(&output);
    let payload = report.get("payload").unwrap();
    expect(
        payload.get("source").unwrap(),
        "sha256",
        &sha256(original.as_bytes()).to_hex(),
    );
    expect(payload, "calibration", "uncalibrated");
    let total = payload.get("total").unwrap();
    expect(total, "dry_tonnes", "3/1");
    expect(total, "recovered_metal", "9/10");
    expect(total, "incremental_contribution", "50/1");
    let pairs = payload.get("allocations").unwrap().as_array().unwrap();
    assert_eq!(pairs.len(), 4);
    for (row, (parcel, route, tonnes, contribution)) in pairs.iter().zip([
        ("clean", "mill", "1/1", "-10/1"),
        ("rich", "mill", "1/2", "40/1"),
        ("clean", "leach", "1/1", "5/1"),
        ("rich", "leach", "1/2", "15/1"),
    ]) {
        expect(row, "parcel", parcel);
        expect(row, "route", route);
        expect(row, "dry_tonnes", tonnes);
        expect(row, "incremental_contribution", contribution);
    }
    let routes = payload.get("routes").unwrap().as_array().unwrap();
    expect(&routes[0], "energy_used_kwh", "30/1");
    expect(&routes[0], "blended_impurity_mass_fraction", "1/50");
    assert_eq!(routes[1].get("energy_used_kwh"), Some(&Json::Null));
    let parcels = payload.get("parcels").unwrap().as_array().unwrap();
    for row in parcels {
        expect(row, "unallocated_dry_tonnes", "0/1");
    }
    assert_eq!(run(input.as_ref(), "json").stdout, output.stdout);
    let md = run(input.as_ref(), "markdown");
    assert!(md.status.success());
    let md = String::from_utf8(md.stdout).unwrap();
    assert!(md.contains("| clean | mill | 1/1 | 1/10 | -10/1 |"));
    assert!(md.contains("| mill | 3/2 / 2/1 | 30/1 / 30/1 | 1/50 / 1/50 |"));

    let scratch = std::env::temp_dir().join(format!(
        "fmine-route-blend-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |file, text| {
        let path = scratch.join(file);
        std::fs::write(&path, text).unwrap();
        blend_envelope(&blend(&root, &path, "json"))
    };
    let market = submit(
        "market.json",
        original.replace(
            r#""market_recovered_metal_cap": "0.9""#,
            r#""market_recovered_metal_cap": "0.4""#,
        ),
    );
    let restricted = market.get("payload").unwrap();
    let total = restricted.get("total").unwrap();
    expect(total, "recovered_metal", "2/5");
    expect(total, "incremental_contribution", "30/1");
    expect(
        &restricted.get("parcels").unwrap().as_array().unwrap()[0],
        "unallocated_dry_tonnes",
        "2/1",
    );
    let idle = submit(
        "idle.json",
        original.replace(
            r#""price_per_metal_unit": "100""#,
            r#""price_per_metal_unit": "0""#,
        ),
    );
    expect(
        idle.get("payload").unwrap().get("total").unwrap(),
        "dry_tonnes",
        "0/1",
    );

    for (file, from, to, code) in [
        (
            "bad_parcel.json",
            r#""parcel": "clean""#,
            r#""parcel": "missing""#,
            "FMINE-REF-107",
        ),
        (
            "duplicate_pair.json",
            r#""parcel": "clean""#,
            r#""parcel": "rich""#,
            "FMINE-REF-109",
        ),
        (
            "missing_energy.json",
            r#", "specific_energy_kwh_per_dry_tonne": "40""#,
            "",
            "FMINE-REF-109",
        ),
        (
            "negative_energy.json",
            r#""energy_budget_kwh": "30""#,
            r#""energy_budget_kwh": "-30""#,
            "FMINE-REF-109",
        ),
    ] {
        assert!(original.contains(from));
        let refusal = submit(file, original.replace(from, to));
        assert_eq!(refusal.get("payload"), Some(&Json::Null));
        expect(refusal.get("refusal").unwrap(), "code", code);
    }
}

#[test]
fn plan_blend_horizon_carries_stock_and_enforces_storage_at_close() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = std::path::Path::new("tests/planning/route_blend_horizon.json");
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let output = blend(&root, input, "json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(blend(&root, input, "json").stdout, output.stdout);
    let report = blend_envelope(&output);
    let payload = report.get("payload").unwrap();
    expect(payload, "schema", "fmine.route-blend-horizon.v1");
    expect(payload, "input_evidence", "synthetic");
    expect(payload, "calibration", "uncalibrated");
    expect(
        payload.get("source").unwrap(),
        "sha256",
        &sha256(original.as_bytes()).to_hex(),
    );
    let total = payload.get("total").unwrap();
    expect(total, "incremental_contribution", "160/1");
    expect(total, "discounted_contribution", "144/1");
    let periods = payload.get("periods").unwrap().as_array().unwrap();
    assert_eq!(periods.len(), 2);
    let assert_stock = |period: &Json, values| {
        let rows = period.get("parcels").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 1);
        for (field, value) in [
            "opening_dry_tonnes",
            "arriving_dry_tonnes",
            "processed_dry_tonnes",
            "closing_dry_tonnes",
        ]
        .into_iter()
        .zip(values)
        {
            expect(&rows[0], field, value);
        }
    };
    assert_stock(&periods[0], ["1/1", "0/1", "0/1", "1/1"]);
    assert_stock(&periods[1], ["1/1", "1/1", "2/1", "0/1"]);
    let markdown = blend(&root, input, "markdown");
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains("| feed | 1/1 | 1/1 | 2/1 | 0/1 | 1/1 |"));

    let scratch = std::env::temp_dir().join(format!("fmine-route-horizon-{}", std::process::id()));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, worksheet: String| {
        let path = scratch.join(name);
        std::fs::write(&path, worksheet).unwrap();
        blend_envelope(&blend(&root, &path, "json"))
    };
    let check = |report: &Json, optimum, first, second| {
        let payload = report.get("payload").unwrap();
        assert_ne!(payload, &Json::Null, "{report:?}");
        expect(
            payload.get("total").unwrap(),
            "discounted_contribution",
            optimum,
        );
        let periods = payload.get("periods").unwrap().as_array().unwrap();
        assert_stock(&periods[0], first);
        assert_stock(&periods[1], second);
    };
    let mandatory = original.replace(
        r#""stockpile_cap_dry_tonnes": "1""#,
        r#""stockpile_cap_dry_tonnes": "0""#,
    );
    let first = ["1/1", "0/1", "1/1", "0/1"];
    let second = ["0/1", "1/1", "1/1", "0/1"];
    check(
        &submit("mandatory.json", mandatory.clone()),
        "82/1",
        first,
        second,
    );
    check(
        &submit(
            "zero_margin.json",
            mandatory
                .replace(
                    r#""price_per_metal_unit": "30""#,
                    r#""price_per_metal_unit": "20""#,
                )
                .replace(
                    r#""price_per_metal_unit": "100""#,
                    r#""price_per_metal_unit": "20""#,
                ),
        ),
        "0/1",
        first,
        second,
    );
    check(
        &submit(
            "loss.json",
            mandatory
                .replace(
                    r#""price_per_metal_unit": "30""#,
                    r#""price_per_metal_unit": "0""#,
                )
                .replace(
                    r#""price_per_metal_unit": "100""#,
                    r#""price_per_metal_unit": "0""#,
                ),
        ),
        "-38/1",
        first,
        second,
    );
    check(
        &submit(
            "later_arrival.json",
            original
                .replace(
                    r#""available_dry_tonnes": "1""#,
                    r#""available_dry_tonnes": "0""#,
                )
                .replace(
                    r#""price_per_metal_unit": "30""#,
                    r#""price_per_metal_unit": "200""#,
                ),
        ),
        "72/1",
        ["0/1"; 4],
        second,
    );
    let impossible = submit(
        "impossible.json",
        mandatory.replacen(r#""dry_tonnes_cap": "2""#, r#""dry_tonnes_cap": "0""#, 1),
    );
    assert_eq!(impossible.get("payload"), Some(&Json::Null));
    let refusal = impossible.get("refusal").unwrap();
    expect(refusal, "code", "FMINE-REF-109");
    assert!(
        refusal
            .get("what")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("no routing")
    );
    let bad_arrival = submit(
        "negative_arrival.json",
        original.replace(r#""dry_tonnes": "1""#, r#""dry_tonnes": "-1""#),
    );
    expect(bad_arrival.get("refusal").unwrap(), "code", "FMINE-REF-109");
}

#[test]
fn plan_blend_values_finite_capacity_options_against_one_baseline() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = std::path::Path::new("tests/planning/route_blend_capacity_options.json");
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let output = blend(&root, input, "json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(blend(&root, input, "json").stdout, output.stdout);
    let envelope = blend_envelope(&output);
    let payload = envelope.get("payload").unwrap();
    expect(
        payload.get("source").unwrap(),
        "sha256",
        &sha256(original.as_bytes()).to_hex(),
    );
    expect(
        payload.get("total").unwrap(),
        "discounted_contribution",
        "82/1",
    );
    let comparison = payload.get("capacity_comparison").unwrap();
    expect(comparison, "base_discounted_contribution", "82/1");
    let best = comparison
        .get("best_positive_options")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(
        best.iter()
            .map(|item| item.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["leased-pad", "shared-pad"]
    );
    let options = comparison.get("options").unwrap().as_array().unwrap();
    assert_eq!(options.len(), 7);
    for (option, (id, gain, cost, net)) in options.iter().zip([
        ("leased-pad", "62/1", "25/1", "37/1"),
        ("shared-pad", "62/1", "25/1", "37/1"),
        ("break-even-pad", "62/1", "62/1", "0/1"),
        ("costly-pad", "62/1", "63/1", "-1/1"),
        ("more-power", "0/1", "5/1", "-5/1"),
        ("more-mill", "0/1", "5/1", "-5/1"),
        ("larger-contract", "0/1", "5/1", "-5/1"),
    ]) {
        expect(option, "id", id);
        expect(option, "break_even_present_value_cost", gain);
        expect(option, "present_value_cost", cost);
        expect(option, "net_present_value_improvement", net);
    }
    expect(&options[0], "discounted_contribution", "144/1");
    expect(&options[0], "base_cap", "0/1");
    expect(&options[0], "new_cap", "1/1");
    expect(&options[4], "capacity_unit", "kWh");
    expect(&options[5], "capacity_unit", "dry_metric_tonne");
    expect(&options[6], "capacity_unit", "g");
    let markdown = blend(&root, input, "markdown");
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains("**Best by net value** leased-pad, shared-pad"));

    let scratch =
        std::env::temp_dir().join(format!("fmine-capacity-options-{}", std::process::id()));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, worksheet: &str| {
        let path = scratch.join(name);
        std::fs::write(&path, worksheet).unwrap();
        blend_envelope(&blend(&root, &path, "json"))
    };
    let usable_stockpile = original.replace(
        r#""stockpile_cap_dry_tonnes": "0""#,
        r#""stockpile_cap_dry_tonnes": "1""#,
    );
    for (name, row, from, to) in [
        (
            "energy.json",
            4,
            r#""energy_budget_kwh": "20""#,
            r#""energy_budget_kwh": "10""#,
        ),
        (
            "mill.json",
            5,
            r#""dry_tonnes_cap": "2""#,
            r#""dry_tonnes_cap": "1""#,
        ),
        (
            "market.json",
            6,
            r#""market_recovered_metal_cap": "2""#,
            r#""market_recovered_metal_cap": "1""#,
        ),
    ] {
        assert!(usable_stockpile.contains(from));
        let report = submit(name, &usable_stockpile.replace(from, to));
        let comparison = report
            .get("payload")
            .unwrap()
            .get("capacity_comparison")
            .unwrap();
        expect(comparison, "base_discounted_contribution", "82/1");
        let options = comparison.get("options").unwrap().as_array().unwrap();
        for (i, option) in options.iter().enumerate() {
            expect(
                option,
                "break_even_present_value_cost",
                if i == row { "62/1" } else { "0/1" },
            );
        }
        expect(&options[row], "net_present_value_improvement", "57/1");
    }
    let combined = usable_stockpile
        .replace(
            r#""energy_budget_kwh": "20""#,
            r#""energy_budget_kwh": "10""#,
        )
        .replace(
            r#""market_recovered_metal_cap": "2""#,
            r#""market_recovered_metal_cap": "1""#,
        );
    let report = submit("two_binding_limits.json", &combined);
    let comparison = report
        .get("payload")
        .unwrap()
        .get("capacity_comparison")
        .unwrap();
    assert!(
        comparison
            .get("best_positive_options")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    for option in comparison.get("options").unwrap().as_array().unwrap() {
        expect(option, "break_even_present_value_cost", "0/1");
    }

    for (name, from, to, code) in [
        (
            "negative_cost.json",
            r#""present_value_cost": "25""#,
            r#""present_value_cost": "-1""#,
            "FMINE-REF-109",
        ),
        (
            "zero_increase.json",
            r#""increase": "10""#,
            r#""increase": "0""#,
            "FMINE-REF-109",
        ),
        (
            "missing_period.json",
            r#""period": "week-2""#,
            r#""period": "missing""#,
            "FMINE-REF-107",
        ),
        (
            "bad_target.json",
            r#""kind": "market""#,
            r#""kind": "market", "route": "mill""#,
            "FMINE-REF-107",
        ),
        (
            "uncapped.json",
            r#", "stockpile_cap_dry_tonnes": "0""#,
            "",
            "FMINE-REF-107",
        ),
        (
            "duplicate.json",
            r#""id": "shared-pad""#,
            r#""id": "leased-pad""#,
            "FMINE-REF-107",
        ),
    ] {
        assert!(original.contains(from));
        let refusal = submit(name, &original.replace(from, to));
        assert_eq!(refusal.get("payload"), Some(&Json::Null));
        expect(refusal.get("refusal").unwrap(), "code", code);
    }
}

#[test]
fn plan_blend_values_a_joint_capacity_package_at_one_price() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = std::path::Path::new("tests/planning/route_blend_capacity_packages.json");
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let output = blend(&root, input, "json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(blend(&root, input, "json").stdout, output.stdout);
    let report = blend_envelope(&output);
    let payload = report.get("payload").unwrap();
    expect(
        payload.get("source").unwrap(),
        "sha256",
        &sha256(original.as_bytes()).to_hex(),
    );
    let comparison = payload.get("capacity_comparison").unwrap();
    expect(comparison, "base_discounted_contribution", "82/1");
    assert_eq!(
        comparison
            .get("best_positive_options")
            .unwrap()
            .as_array()
            .unwrap(),
        &[Json::String("power-and-offtake".into())]
    );
    let options = comparison.get("options").unwrap().as_array().unwrap();
    assert_eq!(options.len(), 3);
    for (option, cost) in options[..2].iter().zip(["12/1", "13/1"]) {
        expect(option, "break_even_present_value_cost", "0/1");
        expect(option, "present_value_cost", cost);
    }
    let package = &options[2];
    expect(package, "kind", "package");
    expect(package, "discounted_contribution", "144/1");
    expect(package, "break_even_present_value_cost", "62/1");
    expect(package, "present_value_cost", "25/1");
    expect(package, "net_present_value_improvement", "37/1");
    let changes = package.get("changes").unwrap().as_array().unwrap();
    assert_eq!(changes.len(), 2);
    for (entry, (kind, unit, current, proposed)) in changes.iter().zip([
        ("route_energy", "kWh", "10/1", "20/1"),
        ("market", "g", "1/1", "2/1"),
    ]) {
        expect(entry, "kind", kind);
        expect(entry, "period", "week-2");
        expect(entry, "capacity_unit", unit);
        expect(entry, "base_cap", current);
        expect(entry, "new_cap", proposed);
    }
    let markdown = blend(&root, input, "markdown");
    assert!(markdown.status.success());
    assert!(String::from_utf8(markdown.stdout).unwrap().contains(
        "| power-and-offtake | Grinding energy · week-2 / mill + Offtake · week-2 | 10/1 → 20/1 kWh, 1/1 → 2/1 g | 62/1 | 25/1 | 37/1 |"
    ));

    let scratch =
        std::env::temp_dir().join(format!("fmine-capacity-package-{}", std::process::id()));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, text: &str| {
        let path = scratch.join(name);
        std::fs::write(&path, text).unwrap();
        blend_envelope(&blend(&root, &path, "json"))
    };
    let (prefix, rest) = original.split_once(r#""changes": ["#).unwrap();
    let (members, suffix) = rest.split_once("\n      ]").unwrap();
    let members: Vec<_> = members
        .trim()
        .lines()
        .map(|line| line.trim().trim_end_matches(','))
        .collect();
    let with_members =
        |items: &[&str]| format!("{prefix}\"changes\":[{}]{suffix}", items.join(","));
    let check_value = |report: Json, gain, net, winning| {
        let comparison = report
            .get("payload")
            .unwrap()
            .get("capacity_comparison")
            .unwrap();
        let package = &comparison.get("options").unwrap().as_array().unwrap()[2];
        expect(package, "break_even_present_value_cost", gain);
        expect(package, "net_present_value_improvement", net);
        assert_eq!(
            comparison
                .get("best_positive_options")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            winning
        );
    };
    check_value(
        submit("reordered.json", &with_members(&[members[1], members[0]])),
        "62/1",
        "37/1",
        1,
    );
    check_value(
        submit(
            "earlier_power.json",
            &with_members(&[&members[0].replace("week-2", "week-1"), members[1]]),
        ),
        "0/1",
        "-25/1",
        0,
    );
    check_value(
        submit(
            "break_even.json",
            &original.replace(
                r#""present_value_cost": "25""#,
                r#""present_value_cost": "62""#,
            ),
        ),
        "62/1",
        "0/1",
        0,
    );
    let throughput =
        r#"{"kind":"route_throughput","period":"week-2","route":"mill","increase":"1"}"#;
    let stockpile = r#"{"kind":"stockpile","parcel":"feed","increase":"1"}"#;
    check_value(
        submit(
            "four_changes.json",
            &with_members(&[members[0], members[1], throughput, stockpile]),
        ),
        "62/1",
        "37/1",
        1,
    );

    for (file, text, code, detail) in [
        (
            "duplicate.json",
            with_members(&[members[0], members[0]]),
            "FMINE-REF-107",
            "repeats",
        ),
        (
            "one_change.json",
            with_members(&[members[0]]),
            "FMINE-REF-107",
            "at least two",
        ),
        (
            "five_changes.json",
            with_members(&[members[0], members[1], throughput, stockpile, members[0]]),
            "FMINE-REF-107",
            "at most 4",
        ),
        (
            "mixed_forms.json",
            original.replace(r#""changes": ["#, r#""kind":"market","changes": ["#),
            "FMINE-REF-107",
            "unknown route-blend field kind",
        ),
        (
            "nested_cost.json",
            with_members(&[
                members[0],
                r#"{"kind":"market","period":"week-2","increase":"1","present_value_cost":"2"}"#,
            ]),
            "FMINE-REF-107",
            "unknown route-blend field present_value_cost",
        ),
        (
            "zero_increase.json",
            with_members(&[
                members[0],
                r#"{"kind":"market","period":"week-2","increase":"0"}"#,
            ]),
            "FMINE-REF-109",
            "positive increase",
        ),
    ] {
        let refusal = submit(file, &text);
        assert_eq!(refusal.get("payload"), Some(&Json::Null));
        let refusal = refusal.get("refusal").unwrap();
        expect(refusal, "code", code);
        assert!(
            refusal
                .get("what")
                .unwrap()
                .as_str()
                .unwrap()
                .contains(detail)
        );
    }
}

#[test]
fn lane_grade_tonnage_binary_reports_exact_partial_bin_for_all_cap_cases() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_grade_tonnage.json";
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let output = lane(&root, input, "json");
    let report = lane_json(&output);
    assert!(output.status.success(), "{report:?}");
    let payload = report.get("payload").unwrap();
    let digest = sha256(original.as_bytes()).to_hex();
    assert_eq!(
        payload
            .get("source")
            .unwrap()
            .get("sha256")
            .unwrap()
            .as_str(),
        Some(digest.as_str())
    );
    let balance = payload.get("capacity_balance").unwrap();
    assert_eq!(
        balance.get("available_dry_tonnes").unwrap().as_str(),
        Some("8/1")
    );
    assert_eq!(
        balance.get("available_recovered_metal").unwrap().as_str(),
        Some("19/10")
    );
    let expect = |result: &Json, mass: &str, metal: &str, grade: &str, share: &str| {
        assert_eq!(result.get("dry_tonnes").unwrap().as_str(), Some(mass));
        assert_eq!(result.get("recovered_metal").unwrap().as_str(), Some(metal));
        let boundary = result.get("boundary").unwrap();
        assert_eq!(
            boundary.get("grade_per_dry_tonne").unwrap().as_str(),
            Some(grade)
        );
        assert_eq!(
            boundary.get("selected_fraction").unwrap().as_str(),
            Some(share)
        );
    };
    // One top-grade tonne yields 0.6 g. At 0.3 g per mid-grade tonne,
    // the amount that exhausts the remaining 0.15 g market capacity is 0.5 t.
    expect(
        balance.get("mill_only").unwrap(),
        "2/1",
        "9/10",
        "2/5",
        "1/3",
    );
    expect(
        balance.get("market_only").unwrap(),
        "3/2",
        "3/4",
        "2/5",
        "1/6",
    );
    assert_eq!(balance.get("both"), balance.get("market_only"));
    fn economic(balance: &Json) -> &Json {
        balance.get("positive_contribution").unwrap()
    }
    let contribution = |allocation: &Json, money: &str| {
        assert_eq!(
            allocation.get("incremental_contribution").unwrap().as_str(),
            Some(money)
        );
    };
    expect(economic(balance), "3/2", "3/4", "2/5", "1/6");
    contribution(economic(balance), "45/1");
    assert_eq!(lane(&root, input, "json").stdout, output.stdout);
    let md = lane(&root, input, "markdown");
    assert!(md.status.success());
    let markdown = String::from_utf8(md.stdout).unwrap();
    assert!(markdown.contains("| Mill and market | 3/2 | 3/4 | 2/5 | 1/6 |"));
    assert!(markdown.contains("| Positive contribution under both caps | 3/2 | 3/4 | 2/5 | 1/6 |"));
    assert!(markdown.contains("**45/1 USD**"));

    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-capacity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let (prefix, _) = original.split_once("\"recovery_curve\":").unwrap();
    let (_, rows) = original.split_once("\"grade_tonnage\":").unwrap();
    let fixed = submit(
        "fixed.json",
        &format!(r#"{prefix}"recovery_fraction":"0.75","grade_tonnage":{rows}"#),
    );
    let fixed_balance = fixed
        .get("payload")
        .unwrap()
        .get("capacity_balance")
        .unwrap();
    expect(
        fixed_balance.get("both").unwrap(),
        "2/1",
        "3/4",
        "2/5",
        "1/3",
    );
    expect(economic(fixed_balance), "2/1", "3/4", "2/5", "1/3");
    contribution(economic(fixed_balance), "35/1");

    for (name, cost, selling, overhead, net) in [
        ("break-even", "30", "0", "10", "30/1"),
        ("loss-making", "35", "0", "10", "25/1"),
        ("selling-cost", "35", "10", "20", "19/1"),
        ("all-idle", "60", "0", "10", "0/1"),
    ] {
        let variant = original
            .replace(
                r#""milling_cost_per_dry_tonne": "20""#,
                &format!(r#""milling_cost_per_dry_tonne": "{cost}""#),
            )
            .replace(
                r#""selling_cost_per_metal_unit": "0""#,
                &format!(r#""selling_cost_per_metal_unit": "{selling}""#),
            )
            .replace(
                r#""fixed_cost_per_period": "10""#,
                &format!(r#""fixed_cost_per_period": "{overhead}""#),
            );
        let variant = submit(&format!("{name}.json"), &variant);
        let variant_balance = variant
            .get("payload")
            .unwrap()
            .get("capacity_balance")
            .unwrap();
        assert_eq!(variant_balance.get("both"), balance.get("both"));
        let selected = economic(variant_balance);
        contribution(selected, net);
        if name == "all-idle" {
            assert_eq!(selected.get("dry_tonnes").unwrap().as_str(), Some("0/1"));
            assert_eq!(
                selected.get("recovered_metal").unwrap().as_str(),
                Some("0/1")
            );
            assert_eq!(selected.get("boundary"), Some(&Json::Null));
        } else {
            expect(selected, "1/1", "3/5", "3/5", "1/1");
        }
    }
    let between_knots = original.replace(
        r#""grade_per_dry_tonne": "0.4", "dry_tonnes": "3""#,
        r#""grade_per_dry_tonne": "0.3", "dry_tonnes": "3""#,
    );
    let interpolated = submit("interpolated.json", &between_knots);
    let joint = interpolated
        .get("payload")
        .unwrap()
        .get("capacity_balance")
        .unwrap()
        .get("both")
        .unwrap();
    expect(joint, "7/4", "3/4", "3/10", "1/4");
    for (i, (from, to, code)) in [
        (
            r#""grade_per_dry_tonne": "0.4", "dry_tonnes""#,
            r#""grade_per_dry_tonne": "0.2", "dry_tonnes""#,
            "FMINE-REF-109",
        ),
        (
            r#""grade_per_dry_tonne": "0.6", "dry_tonnes""#,
            r#""grade_per_dry_tonne": "0.61", "dry_tonnes""#,
            "FMINE-REF-203",
        ),
        (
            r#""dry_tonnes": "4""#,
            r#""dry_tonnes": "0""#,
            "FMINE-REF-109",
        ),
        (
            r#""dry_tonnes": "4""#,
            r#""wet_tonnes": "4""#,
            "FMINE-REF-107",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(original.contains(from));
        let refused = submit(&format!("invalid-{i}.json"), &original.replace(from, to));
        assert_eq!(refused.get("payload"), Some(&Json::Null));
        assert_eq!(
            refused
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some(code)
        );
    }
}

#[test]
fn lane_impurity_blend_binary_prices_dilution_and_respects_all_caps() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_impurity_blend.json";
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let output = lane(&root, input, "json");
    assert!(output.status.success());
    let report = lane_json(&output);
    let payload = report.get("payload").unwrap();
    let digest = sha256(original.as_bytes()).to_hex();
    assert_eq!(
        payload
            .get("source")
            .unwrap()
            .get("sha256")
            .unwrap()
            .as_str(),
        Some(digest.as_str())
    );
    let expect = |report: &Json, [low, high, dry, metal, net]: [&str; 5], assay: Option<&str>| {
        let blend = report
            .get("payload")
            .unwrap()
            .get("impurity_constrained_blend")
            .unwrap();
        assert_eq!(blend.get("element").unwrap().as_str(), Some("As"));
        assert_eq!(
            blend.get("maximum_mass_fraction").unwrap().as_str(),
            Some("1/50")
        );
        match assay {
            Some(value) => assert_eq!(
                blend.get("blended_mass_fraction").unwrap().as_str(),
                Some(value)
            ),
            None => assert_eq!(blend.get("blended_mass_fraction"), Some(&Json::Null)),
        }
        assert_eq!(blend.get("dry_tonnes").unwrap().as_str(), Some(dry));
        assert_eq!(blend.get("recovered_metal").unwrap().as_str(), Some(metal));
        assert_eq!(
            blend.get("incremental_contribution").unwrap().as_str(),
            Some(net)
        );
        let bins = blend.get("selected_bins").unwrap().as_array().unwrap();
        assert_eq!(bins.len(), 2);
        for (bin, tonnes) in bins.iter().zip([low, high]) {
            assert_eq!(
                bin.get("selected_dry_tonnes").unwrap().as_str(),
                Some(tonnes)
            );
        }
    };
    expect(&report, ["4/3", "2/3", "2/1", "8/15", "40/3"], Some("1/50"));
    let physical = payload.get("capacity_balance").unwrap();
    let economic = physical.get("positive_contribution").unwrap();
    assert_eq!(
        economic.get("incremental_contribution").unwrap().as_str(),
        Some("40/1")
    );
    assert_eq!(lane(&root, input, "json").stdout, output.stdout);
    let md = lane(&root, input, "markdown");
    assert!(md.status.success());
    let md = String::from_utf8(md.stdout).unwrap();
    assert!(md.contains("| 1/5 | 4/3 |"));
    assert!(md.contains("| 3/5 | 2/3 |"));
    assert!(md.contains("**40/3 USD**"));

    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-impurity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let market = original.replace(
        r#""market_metal_units_per_period": "0.75""#,
        r#""market_metal_units_per_period": "0.4""#,
    );
    expect(
        &submit("market.json", &market),
        ["1/1", "1/2", "3/2", "2/5", "10/1"],
        Some("1/50"),
    );
    let availability = original.replace(r#""dry_tonnes": "4""#, r#""dry_tonnes": "0.5""#);
    expect(
        &submit("availability.json", &availability),
        ["1/2", "1/4", "3/4", "1/5", "5/1"],
        Some("1/50"),
    );
    let idle = original.replace(
        r#""impurity_mass_fraction": "0.01""#,
        r#""impurity_mass_fraction": "0.03""#,
    );
    expect(&submit("idle.json", &idle), ["0/1"; 5], None);
    let (prefix, _) = original.split_once("\"recovery_curve\":").unwrap();
    let (_, remaining) = original.split_once("\"blend_impurity_limit\":").unwrap();
    let fixed = format!(r#"{prefix}"recovery_fraction":"0.75","blend_impurity_limit":{remaining}"#);
    expect(
        &submit("fixed.json", &fixed),
        ["4/3", "2/3", "2/1", "1/2", "10/1"],
        Some("1/50"),
    );
    for (name, from, to, code) in [
        (
            "missing-assay",
            r#", "impurity_mass_fraction": "0.01""#,
            "",
            "FMINE-REF-107",
        ),
        (
            "bad-assay",
            r#""impurity_mass_fraction": "0.01""#,
            r#""impurity_mass_fraction": "1.01""#,
            "FMINE-REF-109",
        ),
        (
            "bad-total-mass",
            r#""impurity_mass_fraction": "0.01""#,
            r#""impurity_mass_fraction": "1""#,
            "FMINE-REF-109",
        ),
        (
            "bad-element",
            r#""element": "As""#,
            r#""element": "Au""#,
            "FMINE-REF-107",
        ),
    ] {
        assert!(original.contains(from));
        let refused = submit(&format!("{name}.json"), &original.replace(from, to));
        assert_eq!(refused.get("payload"), Some(&Json::Null));
        assert_eq!(
            refused
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some(code)
        );
    }
}

#[test]
fn lane_blend_binary_applies_grinding_energy_alone_or_with_impurity() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_energy_blend.json";
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let output = lane(&root, input, "json");
    assert!(output.status.success());
    let report = lane_json(&output);
    let expect =
        |report: &Json, key: &str, [low, high, net, budget, used, remaining]: [&str; 6]| {
            let blend = report.get("payload").unwrap().get(key).unwrap();
            assert_eq!(
                blend.get("incremental_contribution").unwrap().as_str(),
                Some(net)
            );
            let energy = blend.get("energy").unwrap();
            for (field, value) in [
                ("budget_kwh", budget),
                ("used_kwh", used),
                ("remaining_kwh", remaining),
            ] {
                assert_eq!(energy.get(field).unwrap().as_str(), Some(value));
            }
            let rows = blend.get("selected_bins").unwrap().as_array().unwrap();
            assert_eq!(rows.len(), 2);
            for (row, selected) in rows.iter().zip([low, high]) {
                assert_eq!(
                    row.get("selected_dry_tonnes").unwrap().as_str(),
                    Some(selected)
                );
            }
        };
    let combined = "impurity_constrained_blend";
    expect(
        &report,
        combined,
        ["1/1", "1/2", "10/1", "30/1", "30/1", "0/1"],
    );
    let payload = report.get("payload").unwrap();
    let blend = payload.get(combined).unwrap();
    assert_eq!(
        blend.get("blended_mass_fraction").unwrap().as_str(),
        Some("1/50")
    );
    assert_eq!(blend.get("recovered_metal").unwrap().as_str(), Some("2/5"));
    assert_eq!(lane(&root, input, "json").stdout, output.stdout);
    let md = lane(&root, input, "markdown");
    assert!(md.status.success());
    let md = String::from_utf8(md.stdout).unwrap();
    assert!(md.contains("Grinding uses **30/1 kWh** of **30/1 kWh**, leaving **0/1 kWh**"));

    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-energy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let budget = |value| {
        original.replace(
            r#""blend_energy_budget_kwh": "30""#,
            &format!(r#""blend_energy_budget_kwh": "{value}""#),
        )
    };
    let slack = submit("slack.json", &budget("100"));
    expect(
        &slack,
        combined,
        ["4/3", "2/3", "40/3", "100/1", "40/1", "60/1"],
    );
    let idle = submit("idle.json", &budget("0"));
    expect(&idle, combined, ["0/1"; 6]);
    let idle_blend = idle.get("payload").unwrap().get(combined).unwrap();
    assert_eq!(idle_blend.get("blended_mass_fraction"), Some(&Json::Null));

    let energy_only = original
        .lines()
        .filter(|line| !line.contains("\"blend_impurity_limit\""))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(r#""impurity_mass_fraction": "0.01", "#, "")
        .replace(r#""impurity_mass_fraction": "0.04", "#, "");
    let energy = submit("energy-only.json", &energy_only);
    expect(
        &energy,
        "energy_constrained_blend",
        ["0/1", "3/4", "30/1", "30/1", "30/1", "0/1"],
    );
    assert!(energy.get("payload").unwrap().get(combined).is_none());

    for (name, text, code) in [
        ("negative-budget.json", budget("-1"), "FMINE-REF-109"),
        (
            "negative-rate.json",
            original.replace(
                r#""specific_energy_kwh_per_dry_tonne": "40""#,
                r#""specific_energy_kwh_per_dry_tonne": "-40""#,
            ),
            "FMINE-REF-109",
        ),
        (
            "missing-rate.json",
            original.replace(r#", "specific_energy_kwh_per_dry_tonne": "40""#, ""),
            "FMINE-REF-107",
        ),
        (
            "undeclared-budget.json",
            original
                .lines()
                .filter(|line| !line.contains("\"blend_energy_budget_kwh\""))
                .collect::<Vec<_>>()
                .join("\n"),
            "FMINE-REF-107",
        ),
    ] {
        let refused = submit(name, &text);
        assert_eq!(refused.get("payload"), Some(&Json::Null));
        assert_eq!(
            refused
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some(code)
        );
    }
}

#[test]
fn lane_recovery_curve_binary_distinguishes_exact_crossings_and_support_limits() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_recovery_curve.json";
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let result = lane(&root, input, "json");
    let report = lane_json(&result);
    assert!(result.status.success(), "{report:?}");
    assert_eq!(result.stdout, lane(&root, input, "json").stdout);
    let payload = report.get("payload").unwrap();
    let digest = sha256(original.as_bytes()).to_hex();
    assert_eq!(
        payload
            .get("source")
            .unwrap()
            .get("sha256")
            .unwrap()
            .as_str(),
        Some(digest.as_str())
    );
    let support = payload.get("recovery_curve_support").unwrap();
    assert_eq!(support.get("lower").unwrap().as_str(), Some("1/5"));
    assert_eq!(support.get("upper").unwrap().as_str(), Some("3/5"));
    let rows = payload.get("limiting_cutoffs").unwrap().as_array().unwrap();
    // Between the first two knots recovered grade equals feed grade minus 0.1.
    // The three required recovered grades are 20/100, 30/100 and 20/90.
    let expected = [
        ("3/10", "entire_band_clears"),
        ("2/5", "entire_band_below"),
        ("29/90", "straddles"),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (grade, screen)) in rows.iter().zip(expected) {
        assert_eq!(row.get("cutoff_grade").unwrap().as_str(), Some(grade));
        assert_eq!(
            row.get("cutoff_status").unwrap().as_str(),
            Some("within_support")
        );
        assert_eq!(row.get("feed_grade_screen").unwrap().as_str(), Some(screen));
    }
    let markdown = lane(&root, input, "markdown");
    assert!(markdown.status.success());
    assert!(
        String::from_utf8(markdown.stdout)
            .unwrap()
            .contains("| market | 29/90 | Straddles |")
    );

    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let (before_curve, _) = original.split_once("\"recovery_curve\":").unwrap();
    let declared = before_curve.replace("\"0.3\"", "\"0.32\"");
    let short = submit(
        "short.json",
        &format!(
            r#"{declared}"recovery_curve": [
            {{ "grade_per_dry_tonne": "0.32", "recovery_fraction": "0.75" }},
            {{ "grade_per_dry_tonne": "0.36", "recovery_fraction": "0.75" }}
        ]}}"#
        ),
    );
    let rows = short
        .get("payload")
        .unwrap()
        .get("limiting_cutoffs")
        .unwrap()
        .as_array()
        .unwrap();
    for (row, (status, screen)) in rows.iter().zip([
        ("all_supported_grades_clear", "entire_band_clears"),
        ("no_supported_grade_clears", "entire_band_below"),
        ("all_supported_grades_clear", "entire_band_clears"),
    ]) {
        assert_eq!(row.get("cutoff_grade"), Some(&Json::Null));
        assert_eq!(row.get("cutoff_status").unwrap().as_str(), Some(status));
        assert_eq!(row.get("feed_grade_screen").unwrap().as_str(), Some(screen));
    }
    let constant = submit(
        "constant.json",
        &format!(r#"{before_curve}"recovery_fraction":"0.75"}}"#),
    );
    let market = &constant
        .get("payload")
        .unwrap()
        .get("limiting_cutoffs")
        .unwrap()
        .as_array()
        .unwrap()[2];
    assert_eq!(market.get("cutoff_grade").unwrap().as_str(), Some("8/27"));
    assert_eq!(
        market.get("feed_grade_screen").unwrap().as_str(),
        Some("entire_band_clears")
    );
    assert!(market.get("cutoff_status").is_none());

    for (index, (from, to, code)) in [
        ("\"0.33\"", "\"0.61\"", "FMINE-REF-203"),
        ("\"0.3\"", "\"0.19\"", "FMINE-REF-203"),
        ("\"0.75\"", "\"0.1\"", "FMINE-REF-109"),
        ("\"0.75\"", "\"1.1\"", "FMINE-REF-109"),
        ("\"0.6\"", "\"0.4\"", "FMINE-REF-109"),
        ("\"0.6\"", "\"1000001\"", "FMINE-REF-109"),
        (
            "\"recovery_curve\":",
            "\"recovery_fraction\":\"0.75\",\"recovery_curve\":",
            "FMINE-REF-107",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(original.contains(from));
        let out = submit(
            &format!("invalid-{index}.json"),
            &original.replace(from, to),
        );
        assert_eq!(out.get("payload"), Some(&Json::Null));
        assert_eq!(
            out.get("refusal").unwrap().get("code").unwrap().as_str(),
            Some(code)
        );
    }
}

#[test]
fn lane_cutoff_binary_replays_pinned_symbolic_ratios_and_grade_screens() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_cutoff.json";
    let result = lane(&root, input, "json");
    let report = lane_json(&result);
    assert!(result.status.success(), "{report:?}");
    assert_eq!(result.stdout, lane(&root, input, "json").stdout);
    let p = report.get("payload").unwrap();
    let source = p.get("source").unwrap();
    let bytes = std::fs::read(root.join(input)).unwrap();
    assert_eq!(source.get("bytes").unwrap().u64(), Some(bytes.len() as u64));
    let digest = sha256(&bytes).to_hex();
    assert_eq!(
        source.get("sha256").unwrap().as_str(),
        Some(digest.as_str())
    );
    for (key, expected) in [
        ("input_evidence", "synthetic"),
        ("calibration", "uncalibrated"),
        ("metal", "Au"),
        ("metal_unit", "g"),
        ("ore_mass_basis", "dry_metric_tonne"),
        ("ownership_basis", "100_percent_project"),
    ] {
        assert_eq!(p.get(key).unwrap().as_str(), Some(expected), "{key}");
    }
    let rows = p.get("limiting_cutoffs").unwrap().as_array().unwrap();
    let oracle = include_str!("../../../fixtures/fmine-derive/golden_formulas.txt");
    let expected = [
        ("lane_mine_cutoff", "mine", "entire_band_clears"),
        ("lane_mill_cutoff", "mill", "entire_band_below"),
        ("lane_refinery_cutoff", "market", "straddles"),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (id, case, screen)) in rows.iter().zip(expected) {
        let expected_grade = oracle
            .lines()
            .find_map(|line| {
                let mut cols = line.split('\t');
                if cols.next() == Some("FORMULA") && cols.next() == Some(id) {
                    cols.nth(5)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(row.get("limiting_constraint").unwrap().as_str(), Some(case));
        assert_eq!(
            row.get("cutoff_grade").unwrap().as_str(),
            Some(expected_grade)
        );
        assert_eq!(row.get("feed_grade_screen").unwrap().as_str(), Some(screen));
    }
    let markdown = lane(&root, input, "markdown");
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains("| mine | 1/5 | Entire band clears |"));
    assert!(markdown.contains("| mill | 4/15 | Entire band below |"));
    assert!(markdown.contains("| market | 5/23 | Straddles |"));
    assert!(markdown.contains(&digest));
    let robot = Command::new(binary())
        .args(["robot-docs", "--json"])
        .output()
        .unwrap();
    assert!(robot.status.success());
    assert!(
        parse(&robot.stdout)
            .unwrap()
            .get("payload")
            .unwrap()
            .get("plan_cutoff")
            .is_some()
    );
}

#[test]
fn lane_cutoff_binary_reports_no_grade_when_break_even_exceeds_one_tonne_per_tonne() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-ceiling-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let original = std::fs::read_to_string(root.join("tests/planning/lane_cutoff.json")).unwrap();
    let rows = |name: &str, milling: &str| {
        // Metal in tonnes: the ceiling is one tonne of metal per dry tonne of ore.
        let text = original
            .replace("\"g\"", "\"t\"")
            .replace("\"17.1\"", &format!("\"{milling}\""));
        std::fs::write(scratch.join(name), text).unwrap();
        let report = lane_json(&lane(&scratch, name, "json"));
        assert_eq!(report.get("status").unwrap().as_str(), Some("ok"), "{name}");
        let payload = report.get("payload").unwrap();
        payload
            .get("limiting_cutoffs")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row.get("cutoff_grade").unwrap().clone(),
                    row.get("feed_grade_screen")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                )
            })
            .collect::<Vec<_>>()
    };
    // Milling at 171 a dry tonne needs 171 / (95 x 0.9) = 2 t of metal in each dry tonne at the mine limit, and
    // more at the mill and market limits: no ore can carry it.
    for (grade, screen) in rows("above.json", "171") {
        assert_eq!(grade, Json::Null);
        assert_eq!(screen, "entire_band_below");
    }
    // At 85.5 the mine limit is exactly the ceiling, which ore can carry, and the other two lie above it.
    let at = rows("at.json", "85.5");
    assert_eq!(at[0].0.as_str(), Some("1/1"), "{at:?}");
    assert_eq!((&at[1].0, &at[2].0), (&Json::Null, &Json::Null));
    assert!(at.iter().all(|(_, screen)| screen == "entire_band_below"));
}

#[test]
fn lane_cutoff_binary_preserves_units_and_refuses_incomplete_or_invalid_basis() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-cutoff-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let original = std::fs::read_to_string(root.join("tests/planning/lane_cutoff.json")).unwrap();
    let write = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let grams = write("grams.json", &original);
    let kilos_text = original
        .replace("\"g\"", "\"kg\"")
        .replace("\"100\"", "\"100000\"")
        .replace("\"5\"", "\"5000\"")
        .replace("\"7.5\"", "\"0.0075\"")
        .replace("\"0.2\"", "\"0.0002\"")
        .replace("\"0.24\"", "\"0.00024\"");
    let kilos = write("kilos.json", &kilos_text);
    let parsed_grade = |row: &Json| {
        let (n, d) = row
            .get("cutoff_grade")
            .unwrap()
            .as_str()
            .unwrap()
            .split_once('/')
            .unwrap();
        Rat::new(n.parse().unwrap(), d.parse().unwrap()).unwrap()
    };
    let payload = |json: Json| json.get("payload").unwrap().clone();
    let (grams, kilos) = (payload(grams), payload(kilos));
    assert_ne!(
        grams.get("source").unwrap().get("sha256"),
        kilos.get("source").unwrap().get("sha256")
    );
    for (g, kg) in grams
        .get("limiting_cutoffs")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .zip(kilos.get("limiting_cutoffs").unwrap().as_array().unwrap())
    {
        assert_eq!(
            parsed_grade(kg)
                .checked_mul(Rat::from_integer(1000))
                .unwrap(),
            parsed_grade(g)
        );
        assert_eq!(g.get("feed_grade_screen"), kg.get("feed_grade_screen"));
    }
    let no_band_text = original.replace(
        ",\n  \"feed_grade_per_dry_tonne\": { \"lower\": \"0.2\", \"upper\": \"0.24\" }",
        "",
    );
    let no_band = payload(write("no-band.json", &no_band_text));
    assert_eq!(no_band.get("feed_grade_per_dry_tonne"), Some(&Json::Null));
    for (unit, upper) in [
        ("g", "1000001"),
        ("kg", "1001"),
        ("oz_t", "32151"),
        ("lb_avdp", "2205"),
        ("t", "1.0001"),
    ] {
        let above_mass = original
            .replace("\"g\"", &format!("\"{unit}\""))
            .replace("\"0.24\"", &format!("\"{upper}\""));
        let refusal = write(&format!("mass-{unit}.json"), &above_mass);
        assert_eq!(
            refusal.get("status").unwrap().as_str(),
            Some("refused"),
            "{unit}"
        );
        assert_eq!(
            refusal
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some("FMINE-REF-109")
        );
    }
    let pure = original
        .replace("\"g\"", "\"t\"")
        .replace("\"0.24\"", "\"1\"");
    assert_eq!(
        write("pure.json", &pure).get("status").unwrap().as_str(),
        Some("ok")
    );
    for (index, (old, new, code)) in [
        ("\"0.9\"", "\"1.01\"", "FMINE-REF-109"),
        ("\"7.5\"", "\"0.6\"", "FMINE-REF-109"),
        ("\"0.2\"", "\"0.25\"", "FMINE-REF-109"),
        (
            "\"dry_metric_tonne\"",
            "\"wet_metric_tonne\"",
            "FMINE-REF-105",
        ),
        (
            "\"100_percent_project\"",
            "\"attributable\"",
            "FMINE-REF-105",
        ),
        (
            "\"evidence\": \"synthetic\"",
            "\"evidence\": \"synthetic\", \"evidence\": \"assumed\"",
            "FMINE-REF-107",
        ),
        (
            "\"price_per_metal_unit\"",
            "\"price_per_short_ton\"",
            "FMINE-REF-107",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(original.contains(old));
        let refusal = write(
            &format!("invalid-{index}.json"),
            &original.replace(old, new),
        );
        assert_eq!(refusal.get("status").unwrap().as_str(), Some("refused"));
        assert_eq!(refusal.get("payload"), Some(&Json::Null));
        assert_eq!(
            refusal
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some(code)
        );
    }
}

#[test]
fn schedule_exports_a_source_bound_certificate_with_the_same_value_scale() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let model = "tests/planning/joint_fractional.pcpsp";
    let precedence = "tests/planning/joint_fractional.prec";
    let candidate = "tests/planning/joint_fractional.sol";
    let run = |decimals: &str, extra: &[&str]| {
        Command::new(binary())
            .current_dir(&root)
            .args([
                "plan",
                "schedule",
                "--pcpsp",
                model,
                "--prec",
                precedence,
                "--candidate",
                candidate,
                "--decimals",
                decimals,
                "--json",
            ])
            .args(extra)
            .output()
            .unwrap()
    };
    let expected_inputs = [
        ("candidate", candidate),
        ("pcpsp", model),
        ("precedence", precedence),
    ]
    .map(|(role, path)| {
        InputDigest::new(role, path, sha256(&std::fs::read(root.join(path)).unwrap()))
    });
    let options = [
        "--certificate-currency",
        "USD",
        "--certificate-horizon",
        "tactical",
    ];
    for decimals in ["0", "6"] {
        let baseline = run(decimals, &[]);
        let output = run(decimals, &options);
        assert!(baseline.status.success());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let baseline = parse(&baseline.stdout).unwrap();
        let report = parse(&output.stdout).unwrap();
        let base_payload = baseline.get("payload").unwrap();
        let payload = report.get("payload").unwrap();
        let mut original = payload.as_object().unwrap().to_vec();
        original.retain(|(key, _)| key != "plan_certificate");
        assert_eq!(Json::Object(original), *base_payload);
        assert_eq!(output.stdout, run(decimals, &options).stdout);
        let preview = payload.get("plan_certificate").unwrap();
        assert_eq!(
            preview.get("format").and_then(Json::as_str),
            Some("FMT-PCERT-1")
        );
        let bytes = decode_hex_field(preview.get("bytes_hex").unwrap().as_str().unwrap()).unwrap();
        let cert = PlanCertificate::from_bytes(&bytes).unwrap();
        assert_eq!(
            preview.get("digest").unwrap().as_str().unwrap(),
            cert.certificate_digest.to_hex()
        );
        assert_eq!(cert.inputs, expected_inputs);
        assert_eq!(cert.currency_code, "USD");
        assert_eq!(cert.horizon, PlanHorizon::Tactical);
        assert_eq!(cert.color, PlanColor::Estimated);
        assert_eq!(cert.calibration, PlanCalibrationStatus::Uncalibrated);
        assert!(cert.saa.is_none() && cert.vss.is_none());
        let scale = i128::try_from(cert.value_scale_denominator).unwrap();
        let selected = ratio(
            payload
                .get("selected")
                .unwrap()
                .get("discounted_value")
                .unwrap(),
        );
        let bound = payload.get("bound").unwrap();
        assert_eq!(selected, Rat::from_integer(30));
        assert_eq!(Rat::new(cert.primal_scaled, scale).unwrap(), selected);
        assert_eq!(
            Rat::new(cert.bound.unwrap().upper_scaled, scale).unwrap(),
            ratio(bound.get("value").unwrap())
        );
        assert_eq!(
            Rat::new(cert.opt_gap_numerator.unwrap(), scale).unwrap(),
            ratio(bound.get("gap").unwrap())
        );
    }
    assert_eq!(run("0", &options[..2]).status.code(), Some(2));
    assert_eq!(
        run(
            "0",
            &[
                "--certificate-currency",
                "usd",
                "--certificate-horizon",
                "tactical"
            ]
        )
        .status
        .code(),
        Some(2)
    );
}
