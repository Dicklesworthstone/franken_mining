//! BD-355 through authenticated document registries, two readers, exact Claims and CLI rendering.
//! Every document, issuer and number below is invented; these controls do not close a real-filing gate.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke, run};
use fmine_core::hash::sha256;
use fmine_metrics::comparison::Metric;

const ISSUERS: [&str; 4] = ["peer_a", "peer_b", "peer_c", "peer_d"];
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    metric: Metric,
}

impl Fixture {
    fn new(production: bool) -> Self {
        Self::new_metric(if production {
            Metric::GoldProduction
        } else {
            Metric::Aisc
        })
    }

    fn new_metric(metric: Metric) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-peers-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for directory in [
            "documents",
            "registries/sources",
            "registries/transcriptions",
        ] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let fixture = Self { root, metric };
        let mut bindings = String::from("schema = \"fmine.peer-comparison.bindings.v1\"\n");
        for (i, issuer) in ISSUERS.into_iter().enumerate() {
            let pdf = format!("%PDF-1.7 Invented peer document {issuer}; not a real filing");
            let hash = sha256(pdf.as_bytes()).to_hex();
            fs::write(fixture.root.join(format!("documents/{issuer}.pdf")), &pdf).unwrap();
            fs::write(fixture.root.join(format!("registries/sources/{issuer}.toml")), format!(
                "schema = \"fmine.sources.v1\"\nissuer = \"{issuer}\"\ncorpus_dir = \"documents\"\nretrieved_utc = \"2031-03-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n\
                 [[subject]]\nname = \"Invented {issuer}\"\n\
                 [[document]]\nfile = \"{issuer}.pdf\"\nkind = \"mda\"\nurl = \"https://example.invalid/{issuer}.pdf\"\nurl_basis = \"W1 table\"\n\
                 published_at = \"2031-02-01\"\npublished_at_basis = \"FACT: invented fixture release date\"\npages = 2\nbytes = {}\nsha256 = \"{hash}\"\n", pdf.len())).unwrap();
            let token = if metric.is_production() {
                ["4000.0", "3000.0", "2000.0", "1000.0"][i]
            } else {
                ["1600", "1600", "1600", "1700"][i]
            };
            fixture.write_rows(issuer, token, &["reader_a", "reader_b"]);
            let _ = write!(
                bindings,
                "\n[[issuer]]\nissuer = \"{issuer}\"\ndocument = \"{issuer}.pdf\"\nquantity = \"peer_actual\"\n"
            );
        }
        fs::write(fixture.root.join("peers.toml"), bindings).unwrap();
        fixture
    }

    fn write_rows(&self, issuer: &str, token: &str, readers: &[&str]) {
        let pdf = fs::read(self.root.join(format!("documents/{issuer}.pdf"))).unwrap();
        let hash = sha256(&pdf).to_hex();
        let (denominator, cost) = if self.metric.is_production() {
            ("produced", "")
        } else {
            (
                "sold",
                ";allocation=by_product;credits=separate_line;inventory=included;ga=included;sustaining_leases=included;minesite_exploration=included;reclamation=operating_only;stripping=included;accounting=as_reported",
            )
        };
        let royalties = match self.metric {
            Metric::CopperC1 => ";royalties=excluded",
            Metric::CopperAisc => ";royalties=included",
            _ => "",
        };
        let metric = self.metric.as_str();
        let unit = self.metric.unit();
        let metal = self.metric.metal();
        let basis = format!(
            "metric={metric};scope=company;ownership=attributable;operations=all;metal={metal};denominator={denominator}{cost}{royalties};definition_page=2;definition_table=Invented reconciliation;definition_row=Metric definition;definition_col=2030"
        );
        let mut entries = format!("schema = \"fmine.transcriptions.v1\"\nissuer = \"{issuer}\"\n");
        for reader in readers {
            let _ = write!(
                entries,
                "\n[[entry]]\ndoc = \"{issuer}.pdf\"\ndoc_sha256 = \"{hash}\"\npage = 1\ntable = \"Invented annual results\"\nrow = \"Reported actual\"\ncol = \"2030\"\ntoken = \"{token}\"\nquantity = \"peer_actual\"\nunit = \"{unit}\"\nbasis = \"{basis}\"\nperiod = \"2030FY\"\nreader = \"{reader}\"\n"
            );
        }
        fs::write(self.rows_path(issuer), entries).unwrap();
    }

    fn rows_path(&self, issuer: &str) -> PathBuf {
        self.root
            .join(format!("registries/transcriptions/{issuer}.toml"))
    }

    fn args(&self) -> Vec<String> {
        [
            "compare",
            "--metric",
            self.metric.as_str(),
            "--issuers",
            "peer_a,peer_b,peer_c,peer_d",
            "--period",
            "2030FY",
            "--as-of",
            "2031-03-01",
            "--bindings",
            "peers.toml",
            "--claim-issuer",
            if self.metric.is_production() {
                "peer_a"
            } else {
                "peer_d"
            },
            "--claim",
            "highest",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    fn invoke(&self) -> Envelope {
        let (envelope, format) = invoke(&self.args(), &self.root);
        assert_eq!(format, OutputFormat::Json);
        envelope
    }

    fn ok(&self) -> Envelope {
        let envelope = self.invoke();
        assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
        assert!(envelope.refusal.is_none());
        envelope
    }

    fn change_rows(&self, issuer: &str, old: &str, new: &str) {
        let path = self.rows_path(issuer);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(old), "fixture replacement is anchored");
        fs::write(path, text.replace(old, new)).unwrap();
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

#[test]
fn copper_peer_binary_matches_library_json_markdown_and_refusals() {
    let binary = std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable = std::env::current_exe().unwrap();
            executable
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        });
    for metric in [
        Metric::CopperProduction,
        Metric::CopperC1,
        Metric::CopperAisc,
    ] {
        let fixture = Fixture::new_metric(metric);
        let result = fixture.ok();
        for (format, expected) in [
            ("json", result.to_json()),
            ("markdown", result.rendered_markdown.unwrap()),
        ] {
            let output = std::process::Command::new(&binary)
                .args(fixture.args())
                .args(["--format", format, "--workspace"])
                .arg(&fixture.root)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{expected}\n")
            );
        }
        fixture.change_rows("peer_d", "metal=copper", "metal=gold");
        let refused = fixture.invoke();
        let output = std::process::Command::new(&binary)
            .args(fixture.args())
            .arg("--workspace")
            .arg(&fixture.root)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(i32::from(refused.exit_code))
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{}\n", refused.to_json())
        );
    }
}
