//! BD-061: usable exact cost worksheets through the CLI, with invented declared inputs.
//! These are arithmetic examples, not WGC guidance examples or authenticated issuer filings.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke, parse_args, run};
use fmine_core::hash::sha256;

const REGISTRY: &str = include_str!("../../../registries/metrics.toml");
const SOURCE: &str = "worksheet.toml";
static NEXT: AtomicU64 = AtomicU64::new(0);

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
        original.replace("on_site_mining_costs", "capital_expenditure_non_sustaining"),
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
