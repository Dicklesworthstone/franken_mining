//! BD-355 through authenticated document registries, two readers, exact Claims and CLI rendering.
//! Every document, issuer and number below is invented; these controls do not close a real-filing gate.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::unwrap_used)]

use fmine_cli::run;

include!("../testdata/peer_comparison_fixture.rs");

impl Fixture {
    fn new(production: bool) -> Self {
        Self::new_metric(if production {
            Metric::GoldProduction
        } else {
            Metric::Aisc
        })
    }
}

#[test]
fn public_peer_comparison_authenticates_four_sources_and_retains_exact_claims() {
    let fixture = Fixture::new(false);
    let result = fixture.ok();
    let json = &result.payload_json;
    assert!(json.contains("\"schema\":\"fmine.peer-comparison.v1\""));
    assert!(
        json.contains("\"peer_count\":4"),
        "full declared peer count retained: {json}"
    );
    assert!(json.contains("\"ascending_rank\":{\"min\":1,\"max\":3}"));
    assert!(json.contains("\"ascending_rank\":{\"min\":4,\"max\":4}"));
    assert!(json.contains("\"descending_rank\":{\"min\":1,\"max\":1}"));
    assert!(json.contains("\"verdict\":\"supported\""));
    assert!(
        json.contains("\"num\":\"3199\",\"den\":\"2\""),
        "half-step lower endpoint retained"
    );
    for issuer in ISSUERS {
        let pdf = fs::read(fixture.root.join(format!("documents/{issuer}.pdf"))).unwrap();
        assert!(
            json.contains(&sha256(&pdf).to_hex()),
            "actual source hash for {issuer}"
        );
        assert!(json.contains(&format!(
            "{issuer}.pdf:p1:Invented annual results:Reported actual/2030"
        )));
        assert!(json.contains(&format!(
            "{issuer}.pdf:p2:Invented reconciliation:Metric definition/2030"
        )));
    }
    assert!(
        json.contains("\"canonical_body_hex\":"),
        "full canonical Claim evidence is exported"
    );
    assert_eq!(
        fixture.ok().to_json(),
        result.to_json(),
        "deterministic replay"
    );
    let mut args = fixture.args();
    args.extend(["--format".into(), "markdown".into()]);
    let (markdown, exit) = run(&args, &fixture.root);
    assert_eq!(exit, 0, "{markdown}");
    assert!(markdown.contains("Competition ranks share position for equal values"));
    assert!(markdown.contains("Source and definition provenance"));
    assert!(markdown.contains("no affiliation implied"));
}

#[test]
fn public_peer_gold_production_is_a_positive_comparable_fallback() {
    let fixture = Fixture::new(true);
    let result = fixture.ok();
    assert!(
        result
            .payload_json
            .contains("\"metric\":\"gold-production\"")
    );
    assert!(result.payload_json.contains("\"unit\":\"koz\""));
    assert!(result.payload_json.contains("\"verdict\":\"supported\""));
    assert!(
        !result.payload_json.contains("\"allocation\":"),
        "physical comparison does not invent cost flags"
    );
}

#[test]
fn public_peer_missing_or_incompatible_readings_refuse_the_complete_population() {
    for change in [
        "missing",
        "single",
        "mismatched",
        "denominator",
        "definition",
        "basis_override",
    ] {
        let fixture = Fixture::new(false);
        match change {
            "missing" => fixture.change_rows(
                "peer_d",
                "quantity = \"peer_actual\"",
                "quantity = \"other_actual\"",
            ),
            "single" => fixture.write_rows("peer_d", "1700", &["reader_a"]),
            "mismatched" => {
                fixture.change_rows("peer_d", "allocation=by_product", "allocation=co_product")
            }
            "denominator" => {
                fixture.change_rows("peer_d", "denominator=sold", "denominator=produced")
            }
            "definition" => fixture.change_rows("peer_d", ";definition_page=2", ""),
            "basis_override" => {
                let path = fixture.root.join("peers.toml");
                let mut text = fs::read_to_string(&path).unwrap();
                text.push_str("definition = \"restated_common_basis\"\n");
                fs::write(path, text).unwrap();
            }
            _ => unreachable!(),
        }
        let result = fixture.invoke();
        assert_ne!(result.exit_code, 0, "{change}: {}", result.to_json());
        assert!(
            result.payload_json.contains("\"declared_peer_count\":4"),
            "{change}: complete population remains explicit"
        );
        assert!(result.payload_json.contains("\"peer_d\""));
        assert!(
            result.payload_json.contains("\"comparison\":null"),
            "no partial ranking on refusal"
        );
        if change == "mismatched" {
            assert!(
                result
                    .refusal
                    .unwrap()
                    .what
                    .contains("definition mismatch allocation")
            );
        }
    }
}

#[test]
fn public_peer_document_bytes_publication_and_original_support_are_live() {
    let fixture = Fixture::new(false);
    let path = fixture.root.join("documents/peer_d.pdf");
    let mut bytes = fs::read(&path).unwrap();
    bytes[12] ^= 1;
    fs::write(path, bytes).unwrap();
    let corrupted = fixture.invoke();
    assert_ne!(
        corrupted.exit_code, 0,
        "same-length source mutation must refuse"
    );
    assert!(corrupted.to_json().to_ascii_lowercase().contains("sha256"));

    let fixture = Fixture::new(false);
    let mut args = fixture.args();
    let at = args.iter().position(|s| s == "--as-of").unwrap() + 1;
    args[at] = "2031-01-01".into();
    let (future, _) = invoke(&args, &fixture.root);
    assert_ne!(future.exit_code, 0);
    assert!(
        future
            .refusal
            .unwrap()
            .what
            .contains("after the evidence cut")
    );

    let fixture = Fixture::new(false);
    fixture.change_rows("peer_d", "period = \"2030FY\"", "period = \"2030H1\"");
    assert_ne!(
        fixture.invoke().exit_code,
        0,
        "half-year cannot stand in for annual actual"
    );
}

#[test]
fn public_peer_interpretation_changes_change_the_verdict_and_claim_ids() {
    let fixture = Fixture::new(false);
    let original = fixture.ok();
    fixture.write_rows("peer_d", "1500", &["reader_a", "reader_b"]);
    let changed = fixture.ok();
    assert!(original.payload_json.contains("\"verdict\":\"supported\""));
    assert!(changed.payload_json.contains("\"verdict\":\"unsupported\""));
    assert_ne!(original.payload_json, changed.payload_json);
    assert!(changed.payload_json.contains("\"raw_token\":\"1500\""));
}

#[test]
fn public_peer_parser_requires_explicit_population_and_bounded_claim() {
    let fixture = Fixture::new(false);
    for (key, value) in [
        ("--issuers", "peer_a,peer_a"),
        ("--period", "2030Q4"),
        ("--claim-issuer", "other"),
        ("--claim", "lowest:5"),
        ("--claim", "industry-leading"),
    ] {
        let mut args = fixture.args();
        let at = args.iter().position(|s| s == key).unwrap() + 1;
        args[at] = value.into();
        let (result, _) = invoke(&args, &fixture.root);
        assert_eq!(result.exit_code, 2, "{key}={value}: {}", result.to_json());
    }
}

#[test]
fn copper_peer_commands_preserve_signed_values_and_source_definitions() {
    for metric in [
        Metric::CopperProduction,
        Metric::CopperC1,
        Metric::CopperAisc,
    ] {
        let fixture = Fixture::new_metric(metric);
        if !metric.is_production() {
            for (issuer, token) in ISSUERS.into_iter().zip(["-1.25", "0.00", "2.50", "3.75"]) {
                fixture.write_rows(issuer, token, &["reader_a", "reader_b"]);
            }
        }
        let result = fixture.ok();
        let json = &result.payload_json;
        assert!(json.contains(&format!("\"metric\":\"{}\"", metric.as_str())));
        assert!(json.contains(&format!("\"unit\":\"{}\"", metric.unit())));
        assert!(json.contains("\"metal\":\"copper\""));
        assert!(json.contains("\"peer_count\":4"));
        assert!(json.contains("\"verdict\":\"supported\""));
        if metric.is_production() {
            assert!(!json.contains("\"royalties\":"));
        } else {
            assert!(json.contains("\"raw_token\":\"-1.25\""));
            assert!(json.contains("\"raw_token\":\"0.00\""));
            assert!(json.contains("\"num\":\"-251\",\"den\":\"200\""));
            assert!(json.contains("\"royalties\":"));
        }
        assert_eq!(fixture.ok().to_json(), result.to_json());
    }
}

#[test]
fn copper_peer_mismatches_refuse_without_dropping_any_declared_issuer() {
    for metric in [
        Metric::CopperProduction,
        Metric::CopperC1,
        Metric::CopperAisc,
    ] {
        let denominator = if metric.is_production() {
            "produced"
        } else {
            "sold"
        };
        for (old, new) in [
            ("metal=copper".to_string(), "metal=gold".to_string()),
            (
                format!("unit = \"{}\"", metric.unit()),
                "unit = \"koz\"".to_string(),
            ),
            (
                format!("denominator={denominator}"),
                "denominator=unstated".to_string(),
            ),
            (
                "period = \"2030FY\"".to_string(),
                "period = \"2030H1\"".to_string(),
            ),
            (
                "quantity = \"peer_actual\"".to_string(),
                "quantity = \"other_actual\"".to_string(),
            ),
        ] {
            let fixture = Fixture::new_metric(metric);
            fixture.change_rows("peer_d", &old, &new);
            let result = fixture.invoke();
            assert_ne!(
                result.exit_code,
                0,
                "{metric:?}: {old}: {}",
                result.to_json()
            );
            assert!(result.payload_json.contains("\"declared_peer_count\":4"));
            assert!(result.payload_json.contains("\"comparison\":null"));
        }
        let fixture = Fixture::new_metric(metric);
        let mut args = fixture.args();
        let at = args.iter().position(|s| s == "--as-of").unwrap() + 1;
        args[at] = "2031-01-31".into();
        let (result, _) = invoke(&args, &fixture.root);
        assert_eq!(result.refusal.unwrap().code, "FMINE-REF-108");
    }
    for (old, new, reason) in [
        (
            ";royalties=excluded",
            "",
            "missing definition field royalties",
        ),
        (
            "royalties=excluded",
            "royalties=included",
            "definition mismatch royalties",
        ),
        (
            "metric=copper-c1",
            "metric=copper-aisc",
            "unsupported definition metric",
        ),
    ] {
        let fixture = Fixture::new_metric(Metric::CopperC1);
        fixture.change_rows("peer_d", old, new);
        let result = fixture.invoke();
        assert_ne!(result.exit_code, 0);
        assert!(result.refusal.unwrap().what.contains(reason));
        assert!(result.payload_json.contains("\"declared_peer_count\":4"));
    }
    let fixture = Fixture::new_metric(Metric::CopperProduction);
    fixture.write_rows("peer_d", "-0.01", &["reader_a", "reader_b"]);
    assert_ne!(
        fixture.invoke().exit_code,
        0,
        "negative copper output must refuse"
    );
}
