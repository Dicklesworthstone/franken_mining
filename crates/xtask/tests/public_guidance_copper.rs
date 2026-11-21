//! DESIGN (BD-342): invented registered copper filings exercise the actual source-to-report path.
//! Determinism: Exact
//! The exact expected bounds below come from displayed half-step arithmetic, independently of the kernel.

#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;
use fmine_evidence::{KnowledgeState, Refusal};
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
use fmine_metrics::capacity::{CapacityVerdict, PhysicalKind};
use fmine_metrics::guidance::{
    EnvelopeTier, GuidanceFamily, HistoryVerdict, QuantityKind, quarters,
};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::guidance::{
    self, GuidanceCompletionReport,
    capacity::{TechnicalAssessment, TechnicalComparison, TechnicalScenarioKind},
};
use fmine_public::sources::{registry_path, transcriptions_path};

const ISSUER: &str = "copper_completion_fixture";
const AS_OF: &str = "2030-07-31";

fn document(file: &str) -> Vec<u8> {
    format!("%PDF-1.7 Invented {file}; synthetic copper completion controls.").into_bytes()
}

fn register_document(root: &Path, file: &str, date: &str, kind: &str) {
    let bytes = document(file);
    std::fs::write(root.join("corpus").join(file), &bytes).unwrap();
    let path = registry_path(root, ISSUER);
    let mut registry = std::fs::read_to_string(&path).unwrap();
    write!(registry, "\n[[document]]\nfile = \"{file}\"\nkind = \"{kind}\"\nurl = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\npublished_at = \"{date}\"\npublished_at_basis = \"FACT: invented publication date\"\npages = 1\nbytes = {}\nsha256 = \"{}\"\n", bytes.len(), sha256(&bytes).to_hex()).unwrap();
    std::fs::write(path, registry).unwrap();
}

fn entry(file: &str, suffix: &str, token: &str, unit: &str, basis: &str, reader: &str) -> String {
    format!(
        "\n[[entry]]\ndoc = \"{file}\"\ndoc_sha256 = \"{}\"\npage = 1\ntable = \"Invented annual guidance\"\nrow = \"Copper Mine\"\ncol = \"{suffix}\"\ntoken = \"{token}\"\nquantity = \"guidance_copper_mine_{suffix}\"\nunit = \"{unit}\"\nbasis = \"{basis}\"\nperiod = \"2030FY\"\nreader = \"{reader}\"\n",
        sha256(&document(file)).to_hex()
    )
}

fn guidance_entries(file: &str, basis: &str) -> String {
    let mut out = String::new();
    for reader in ["reader-a", "reader-b"] {
        out.push_str(&entry(
            file,
            "production_kt",
            "100 - 120",
            "kt",
            basis,
            reader,
        ));
        out.push_str(&entry(
            file,
            "aisc_usd_per_lb",
            "3.00 - 3.50",
            "usd_per_lb",
            &format!("{basis};aisc=issuer-reported-per-sold-lb"),
            reader,
        ));
    }
    out
}

fn technical_entries(file: &str, scenario: &str, explicit: bool) -> String {
    let mut out = String::new();
    let basis = if explicit {
        "100%;bound=explicit-upper;metal=copper".to_string()
    } else {
        format!("100%;scenario={scenario};metal=copper")
    };
    for (suffix, token, unit) in [
        ("remaining_dry_feed_mt", "10", "dry_mt"),
        ("head_grade_percent", "2", "percent_dry_mass"),
        ("recovery_percent", "90", "percent"),
    ] {
        for reader in ["reader-a", "reader-b"] {
            out.push_str(
                &entry(file, suffix, token, unit, &basis, reader)
                    .replace(
                        "guidance_copper_mine_",
                        if explicit {
                            "capacity_copper_mine_"
                        } else {
                            "technical_copper_mine_"
                        },
                    )
                    .replace(
                        "Invented annual guidance",
                        if explicit {
                            "Explicit maximum limits"
                        } else {
                            "Invented technical scenario"
                        },
                    )
                    .replace(
                        "2030FY",
                        if explicit {
                            "2030-07/2030-12"
                        } else {
                            "2030H2"
                        },
                    ),
            );
        }
    }
    out
}

fn technical_comparison(report: &GuidanceCompletionReport) -> &TechnicalComparison {
    let TechnicalAssessment::Compared(comparison) = &report.technical else {
        panic!(
            "expected a source-bound T2 copper comparison: {:?}",
            report.technical
        );
    };
    comparison
}

fn technical_refusal(report: &GuidanceCompletionReport) -> &Refusal {
    let TechnicalAssessment::Indeterminate { reason } = &report.technical else {
        panic!(
            "expected an indeterminate technical comparison: {:?}",
            report.technical
        );
    };
    reason
}

fn write_entries(root: &Path, entries: &str) {
    std::fs::write(
        transcriptions_path(root, ISSUER),
        format!("schema = \"fmine.transcriptions.v1\"\nissuer = \"{ISSUER}\"\n{entries}"),
    )
    .unwrap();
}

fn sheets(
    period: (&str, &str, &str),
    share: &str,
    values: [&str; 3],
    site_index: u8,
    fault: &str,
) -> Vec<MinimalSheet> {
    let (year, months, end) = period;
    let text = |s: &str| MinimalCell::Text(s.into());
    let number = |s: &str| MinimalCell::Styled {
        raw: s.into(),
        xf: 1,
    };
    let heading = format!("For the {months} months ended {end},");
    let subject = format!("Copper Mine ({share})");
    let rate = match fault {
        "signed_cost" => format!("-{}", values[2]),
        "zero_cost" => "0".into(),
        _ => values[2].into(),
    };
    let full = values[0].parse::<u64>().unwrap() * if share == "50%" { 2 } else { 1 };
    let sales_label = match fault {
        "unsupported_sales_unit" => "Sales (kilograms)",
        "production_as_sales" => "Production (millions of pounds)",
        "sales_kt" => "Sales (Thousands of tonnes)",
        "aliased_sales_kt" if year == "2030" => "Sales (Thousands of tonnes)",
        "mixed_sales_units" if year == "2030" || end == "September 30" => {
            "Sales (Thousands of tonnes)"
        }
        _ => "Sales (millions of pounds)",
    };
    let mut sheets = vec![
        MinimalSheet {
            name: if months == "three" { "QTD" } else { "YTD" }.into(),
            hidden: false,
            cells: vec![
                ("A3".into(), text(&heading)),
                (
                    "B3".into(),
                    MinimalCell::Styled {
                        raw: year.into(),
                        xf: 0,
                    },
                ),
                ("A5".into(), text(&subject)),
                ("B4".into(), text("Copper produced (kt 100%)")),
                ("C4".into(), text("Copper produced (kt attributable)")),
                ("D4".into(), text("All-in sustaining costs ($/lb)")),
                ("B5".into(), number(&full.to_string())),
                ("C5".into(), number(values[0])),
                ("D5".into(), number(&rate)),
            ],
        },
        MinimalSheet {
            name: format!("Minestats{site_index}"),
            hidden: false,
            cells: vec![
                (
                    "B1".into(),
                    text(if fault == "sales_basis" {
                        "Copper Mine (60%)"
                    } else {
                        &subject
                    }),
                ),
                ("A2".into(), text(&heading)),
                (
                    "B2".into(),
                    MinimalCell::Styled {
                        raw: year.into(),
                        xf: 0,
                    },
                ),
                ("A3".into(), text(sales_label)),
                ("B3".into(), number(values[1])),
            ],
        },
    ];
    if fault == "ambiguous_sales" {
        sheets[1].cells.extend([
            ("A4".into(), text("Sales (Thousands of tonnes)")),
            ("B4".into(), number(values[1])),
        ]);
    }
    sheets
}

fn workspace(tag: &str, share: &str, fault: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "fmine-copper-completion-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    for dir in ["corpus", "registries/sources", "registries/transcriptions"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    let mut registry = format!(
        "schema = \"fmine.sources.v1\"\nissuer = \"{ISSUER}\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2030-07-31T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n[[subject]]\nname = \"Copper Mine\"\n"
    );
    let historical_fault = if [
        "sales_kt",
        "aliased_sales_kt",
        "mixed_sales_units",
        "signed_cost",
        "zero_cost",
    ]
    .contains(&fault)
    {
        fault
    } else {
        ""
    };
    if fault == "aliased_sales_kt" {
        registry.push_str("\n[[mine_stats_label]]\nlabel = \"Sales (millions of pounds)\"\n\n[[mine_stats_label]]\nlabel = \"Sales (Thousands of tonnes)\"\n");
        registry.push_str("\n[[mine_stats_label_alias]]\nlabel = \"Sales (millions of pounds)\"\nreads_as = \"Sales (Thousands of tonnes)\"\nfirst = \"2029Q1\"\nlast = \"2029Q4\"\nevidence = \"SYNTHETIC-STALE-SALES-LABEL\"\n");
    }
    let mut q4 = sheets(
        ("2029", "three", "December 31"),
        share,
        ["35", "50", "3"],
        1,
        historical_fault,
    );
    q4.extend(sheets(
        ("2029", "twelve", "December 31"),
        share,
        ["110", "160", "3.5"],
        2,
        historical_fault,
    ));
    for (release, date, sheets) in [
        (
            "2029Q2",
            "2029-07-15",
            sheets(
                ("2029", "six", "June 30"),
                share,
                ["40", "60", "3"],
                1,
                historical_fault,
            ),
        ),
        (
            "2029Q3",
            "2029-10-15",
            sheets(
                ("2029", "three", "September 30"),
                share,
                ["35", "50", "3"],
                1,
                historical_fault,
            ),
        ),
        ("2029Q4", "2030-02-15", q4),
        (
            "2030Q2",
            "2030-07-15",
            sheets(
                ("2030", "six", "June 30"),
                share,
                ["40", "60", "4"],
                1,
                fault,
            ),
        ),
    ] {
        let file = format!("{release}.xlsx");
        let bytes = build_minimal_xlsx_styled(
            &sheets,
            Some(&MinimalStyles {
                num_fmts: vec![(164, "0.00".into())],
                xfs: vec![0, 164],
            }),
        );
        std::fs::write(root.join("corpus").join(&file), &bytes).unwrap();
        write!(registry, "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{release}\"\npath = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\npublished_at = \"{date}\"\npublished_at_basis = \"FACT: invented publication date\"\n", bytes.len(), sha256(&bytes).to_hex()).unwrap();
    }
    std::fs::write(registry_path(&root, ISSUER), registry).unwrap();
    register_document(&root, "Guidance.pdf", "2030-01-15", "mda");
    let entries = guidance_entries(
        "Guidance.pdf",
        if share == "50%" {
            "attributable:1/2"
        } else {
            "100%"
        },
    );
    let entries = match fault {
        "signed_cost" => entries.replace("3.00 - 3.50", "-3.50 - -3.00"),
        "zero_cost" => entries.replace("3.00 - 3.50", "-0.50 - 0.50"),
        _ => entries,
    };
    write_entries(&root, &entries);
    root
}

fn run(root: &Path) -> Result<GuidanceCompletionReport, fmine_evidence::Refusal> {
    guidance::run_public(root, ISSUER, "copper-mine", "2030Q2", AS_OF)
}

fn interval(lo: (i128, i128), hi: (i128, i128)) -> Interval {
    Interval {
        lo: Rat::new(lo.0, lo.1).unwrap(),
        hi: Rat::new(hi.0, hi.1).unwrap(),
        lo_strict: false,
        hi_strict: false,
    }
}

fn arguments() -> Vec<String> {
    [
        "guidance",
        "completion",
        "--issuer",
        ISSUER,
        "--site",
        "copper-mine",
        "--through",
        "2030Q2",
        "--as-of",
        AS_OF,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[test]
fn copper_source_completion_keeps_exact_units_rounding_and_checked_history() {
    for share in ["50%", "100%"] {
        let root = workspace("known-answer", share, "");
        let report = run(&root).unwrap();
        let i = &report.inputs;
        assert_eq!(i.ytd_production.value.kind, QuantityKind::ProductionKt);
        assert_eq!(i.ytd_sold.value.kind, QuantityKind::SoldMillionLb);
        assert_eq!(i.ytd_aisc.value.kind, QuantityKind::AiscUsdPerSoldLb);
        assert_eq!(
            report.completion.remaining_production.value.interval,
            interval((11999, 200), (16001, 200))
        );
        assert_eq!(
            i.remaining_sold.value.interval,
            interval((9999, 100), (10001, 100))
        );
        // Independently evaluated corners of r*S, A*R+(A-r)*S and A+(A-r)*S/R. No gold /1000 factor.
        assert_eq!(
            report.completion.ytd_aisc_spend.value.interval,
            interval((9587201, 40000), (9612801, 40000))
        );
        assert_eq!(
            report.completion.remaining_aisc_spend.value.interval,
            interval((9586599, 40000), (12813499, 40000))
        );
        assert_eq!(
            report.completion.remaining_aisc_rate.value.interval,
            interval((9586599, 3999600), (12813499, 4000400))
        );
        assert_eq!(
            (report.history.tier, report.history.verdict),
            (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
        );
        let source = &report.history_sources[0];
        let witness = &report.history.checks[0].witness.as_ref().unwrap().value;
        let original = quarters::QuarterlyHistory {
            id: source.id.clone(),
            quarters: source.quarters.clone(),
        };
        assert!(quarters::check_quarter_witness(i, &original, witness).unwrap());
        let sum = witness
            .quarters
            .iter()
            .fold(Rat::from_integer(0), |sum, q| {
                assert_eq!(q.family, GuidanceFamily::Copper);
                assert_eq!(q.spend_usd_m, q.sold.checked_mul(q.aisc).unwrap());
                sum.checked_add(q.spend_usd_m).unwrap()
            });
        assert_eq!(sum, witness.completion.remaining_spend_usd_m);
        assert_eq!(witness.completion.family, GuidanceFamily::Copper);
        assert_eq!(i.remaining_sold.parents.len(), 2);
        let json = report.to_json();
        for key in [
            "production_kt",
            "sold_million_lb",
            "aisc_usd_per_lb",
            "issuer-reported-per-sold-lb",
            "2030-07-15",
            "2030Q2.xlsx:Minestats1!B3",
        ] {
            assert!(json.contains(key), "missing {key}: {json}");
        }
        assert!(!json.contains("_koz") && !json.contains("_per_oz"));
        assert!(!json.contains("sold ounces"));
        assert!(
            json.contains(
                &sha256(&std::fs::read(root.join("corpus/2030Q2.xlsx")).unwrap()).to_hex()
            )
        );
        assert!(matches!(
            report.technical,
            TechnicalAssessment::Indeterminate { .. }
        ));
        assert!(json.contains("no registered technical"));
        assert_eq!(json, run(&root).unwrap().to_json());
    }
}

#[test]
fn signed_and_zero_copper_costs_reach_the_cli_with_exact_bounds_and_historical_witnesses() {
    for (case, ytd, spend, rate, shown) in [
        (
            "signed_cost",
            interval((-9612801, 40000), (-9587201, 40000)),
            interval((-12813499, 40000), (-9586599, 40000)),
            interval((-12813499, 4000400), (-9586599, 3999600)),
            "−320.337475",
        ),
        (
            "zero_cost",
            interval((-12001, 40000), (12001, 40000)),
            interval((-3212301, 40000), (3212301, 40000)),
            interval((-2891, 3600), (2891, 3600)),
            "−80.307525",
        ),
    ] {
        let root = workspace(case, "50%", case);
        let report = run(&root).unwrap();
        // Independent half-step arithmetic: signed_cost reverses the prior known-answer rate
        // signs; zero_cost has r=[-0.005,0.005], A=[-0.5,0.5], S=[59.995,60.005], R=[99.99,100.01].
        assert_eq!(report.completion.ytd_aisc_spend.value.interval, ytd);
        assert_eq!(report.completion.remaining_aisc_spend.value.interval, spend);
        assert_eq!(report.completion.remaining_aisc_rate.value.interval, rate);
        assert_eq!(report.history.verdict, HistoryVerdict::WithinEnvelope);
        assert_eq!(report.history_sources.len(), 1);
        let source = &report.history_sources[0];
        let history = quarters::QuarterlyHistory {
            id: source.id.clone(),
            quarters: source.quarters.clone(),
        };
        let witness = &report.history.checks[0].witness.as_ref().unwrap().value;
        assert!(quarters::check_quarter_witness(&report.inputs, &history, witness).unwrap());
        assert!(report.inputs.ytd_aisc.value.interval.lo.signum() < 0);
        assert!(
            !report
                .history_exclusions
                .iter()
                .any(|e| e.code == "FMINE-REF-109")
        );

        let (envelope, _) = fmine_cli::invoke(&arguments(), &root);
        assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
        assert_eq!(envelope.payload_json, report.to_json());
        for source in ["Guidance.pdf", "2030Q2.xlsx:YTD!D5", "2029Q3.xlsx:QTD!D5"] {
            assert!(envelope.payload_json.contains(source));
        }
        assert!(
            envelope
                .payload_json
                .contains(&format!("\"num\":\"{}\"", spend.lo.num()))
        );
        let mut markdown = arguments();
        markdown.extend(["--format".into(), "markdown".into()]);
        let (text, code) = fmine_cli::run(&markdown, &root);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains(shown), "{text}");
        assert!(text.contains("USD per sold lb") && text.contains("Checked witness"));
        assert_eq!(report.to_json(), run(&root).unwrap().to_json());
    }
}

#[test]
fn copper_kt_sales_and_registered_label_corrections_use_exact_source_bound_conversion() {
    for format in ["sales_kt", "aliased_sales_kt"] {
        let root = workspace(format, "50%", format);
        let report = run(&root).unwrap();
        // 1 kt = 100000000/45359237 million lb. These bounds independently scale the
        // displayed 0.01 kt half-steps before subtraction and multiplication by USD/lb.
        assert_eq!(
            report.inputs.ytd_sold.value.interval,
            interval((5999500000, 45359237), (545500000, 4123567))
        );
        assert_eq!(
            report.inputs.remaining_sold.value.interval,
            interval((909000000, 4123567), (10001000000, 45359237))
        );
        assert_eq!(
            report.completion.ytd_aisc_spend.value.interval,
            interval((23968002500, 45359237), (2184727500, 4123567))
        );
        assert_eq!(
            report.completion.remaining_aisc_spend.value.interval,
            interval((2178772500, 4123567), (32033747500, 45359237))
        );
        assert_eq!(
            report.completion.remaining_aisc_rate.value.interval,
            interval((9586599, 3999600), (12813499, 4000400))
        );
        assert_eq!(
            report.completion.remaining_production.value.interval,
            interval((11999, 200), (16001, 200))
        );
        assert_eq!(
            (report.history.tier, report.history.verdict),
            (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
        );
        assert_eq!(
            report.inputs.ytd_sold.metric_key,
            "guidance.copper-sales.kt-to-million-lb/1"
        );
        assert!(
            report
                .inputs
                .ytd_sold
                .value
                .definition
                .contains("100000000/45359237")
        );
        for window in &report.history_sources {
            for (quarter, sources) in window
                .quarters
                .iter()
                .zip(window.source_claims.as_chunks::<3>().0)
            {
                let original = &sources[1];
                original.claim.check_id().unwrap();
                assert_eq!(original.key.unit, "kt");
                assert_eq!(original.key.line.label, "site::sales (thousands of tonnes)");
                assert_eq!(quarter.sold.parents, vec![original.claim.claim_id]);
                assert_eq!(quarter.sold.anchors, original.claim.anchors);
                assert_eq!(
                    quarter.sold.metric_key,
                    "guidance.copper-sales.kt-to-million-lb/1"
                );
                if format == "aliased_sales_kt" {
                    assert_eq!(
                        original.label_as_printed.as_deref(),
                        Some("Sales (millions of pounds)")
                    );
                    assert_eq!(
                        original.unit_evidence.as_deref(),
                        Some("SYNTHETIC-STALE-SALES-LABEL")
                    );
                }
            }
        }
        let source = &report.history_sources[0];
        let witness = &report.history.checks[0].witness.as_ref().unwrap().value;
        let original = quarters::QuarterlyHistory {
            id: source.id.clone(),
            quarters: source.quarters.clone(),
        };
        assert!(quarters::check_quarter_witness(&report.inputs, &original, witness).unwrap());
        for quarter in &witness.quarters {
            assert_eq!(
                quarter.spend_usd_m,
                quarter.sold.checked_mul(quarter.aisc).unwrap()
            );
        }
        let json = report.to_json();
        assert!(json.contains("\"conversion\":{\"rule\":\"guidance.copper-sales.kt-to-million-lb/1\",\"source_unit\":\"kt\",\"target_unit\":\"Mlb\",\"factor\":\"100000000/45359237\"}"));
        assert!(json.contains("2030Q2.xlsx:Minestats1!B3"));
        if format == "aliased_sales_kt" {
            assert!(json.contains("SYNTHETIC-STALE-SALES-LABEL"));
            assert!(json.contains("Sales (millions of pounds)"));
        }
    }
}

#[test]
fn copper_mixed_sales_units_convert_each_original_release_before_cost_and_history() {
    let root = workspace("mixed-units", "50%", "mixed_sales_units");
    let report = run(&root).unwrap();
    assert_eq!(
        report.inputs.ytd_sold.value.interval,
        interval((5999500000, 45359237), (545500000, 4123567))
    );
    // Prior-year FY and YTD are already Mlb; current YTD is kt. They cannot share a guessed unit.
    assert_eq!(
        report.inputs.remaining_sold.value.interval,
        interval((9999, 100), (10001, 100))
    );
    assert_eq!(
        report.completion.remaining_aisc_spend.value.interval,
        interval((68871889299, 412356700), (234683054969, 824713400))
    );
    assert_eq!(
        report.completion.remaining_aisc_rate.value.interval,
        interval((22957296433, 13743848811), (234683054969, 82479587134))
    );
    let source = &report.history_sources[0];
    assert_eq!(source.quarters[0].id, "2029Q3");
    assert_eq!(source.quarters[1].id, "2029Q4");
    assert_eq!(
        source.quarters[0].sold.value.interval,
        interval((454500000, 4123567), (5000500000, 45359237))
    );
    assert_eq!(
        source.quarters[1].sold.value.interval,
        interval((9999, 200), (10001, 200))
    );
    assert_eq!(source.source_claims[1].key.unit, "kt");
    assert_eq!(source.source_claims[4].key.unit, "Mlb");
    assert_eq!(
        source.quarters[1].sold.metric_key,
        "site::sales (millions of pounds)"
    );
    // Those two original quarters sum to about 160 Mlb, which cannot fit the explicit 100 Mlb scenario.
    assert_eq!(
        (report.history.tier, report.history.verdict),
        (EnvelopeTier::T1, HistoryVerdict::ExceedsEnvelope)
    );
    assert!(report.history.checks[0].witness.is_none());
}

#[test]
fn copper_guidance_refuses_unsupported_sales_units_ambiguity_and_ownership() {
    for (fault, code) in [
        ("unsupported_sales_unit", "FMINE-REF-201"),
        ("production_as_sales", "FMINE-REF-201"),
        ("ambiguous_sales", "FMINE-REF-104"),
        ("sales_basis", "FMINE-REF-105"),
    ] {
        let root = workspace(fault, "50%", fault);
        assert_eq!(run(&root).unwrap_err().code, code);
    }
    for (old, new, code) in [
        ("unit = \"kt\"", "unit = \"koz\"", "FMINE-REF-105"),
        (
            ";aisc=issuer-reported-per-sold-lb",
            ";aisc=issuer-reported-per-sold-oz",
            "FMINE-REF-107",
        ),
        (
            "attributable:1/2;aisc=",
            "attributable:3/5;aisc=",
            "FMINE-REF-105",
        ),
    ] {
        let root = workspace("bad-guidance", "50%", "");
        let path = transcriptions_path(&root, ISSUER);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(old));
        std::fs::write(path, text.replace(old, new)).unwrap();
        assert_eq!(run(&root).unwrap_err().code, code);
    }
}

#[test]
fn copper_guidance_honors_latest_documents_reader_agreement_and_publication_cut() {
    for pending in [false, true] {
        let root = workspace("later-guidance", "50%", "");
        register_document(&root, "Later.pdf", "2030-07-01", "mda");
        let mut rows = guidance_entries("Guidance.pdf", "attributable:1/2");
        rows.push_str(&entry(
            "Later.pdf",
            "production_kt",
            "110 - 130",
            "kt",
            "attributable:1/2",
            "reader-a",
        ));
        if !pending {
            rows.push_str(&entry(
                "Later.pdf",
                "production_kt",
                "110 - 130",
                "kt",
                "attributable:1/2",
                "reader-b",
            ));
        }
        write_entries(&root, &rows);
        assert_eq!(run(&root).unwrap_err().code, "FMINE-REF-202");
    }
    let root = workspace("future-guidance", "50%", "");
    let baseline = run(&root).unwrap().digest;
    register_document(&root, "Future.pdf", "2030-08-01", "mda");
    let rows = guidance_entries("Guidance.pdf", "attributable:1/2")
        + &entry(
            "Future.pdf",
            "production_kt",
            "999",
            "kt",
            "attributable:1/2",
            "reader-a",
        );
    write_entries(&root, &rows);
    assert_eq!(baseline, run(&root).unwrap().digest);
    let before =
        guidance::run_public(&root, ISSUER, "copper-mine", "2030Q2", "2030-07-14").unwrap_err();
    assert_eq!(before.code, "FMINE-REF-201");
    std::fs::write(root.join("corpus/2030Q2.xlsx"), b"altered workbook").unwrap();
    assert_eq!(run(&root).unwrap_err().code, "FMINE-REF-505");
}

#[test]
fn copper_completion_does_not_choose_between_commodity_families_or_disagreeing_readers() {
    let root = workspace("mixed-guidance", "50%", "");
    let mut rows = guidance_entries("Guidance.pdf", "attributable:1/2");
    for reader in ["reader-a", "reader-b"] {
        rows.push_str(&entry(
            "Guidance.pdf",
            "production_koz",
            "100 - 120",
            "koz",
            "attributable:1/2",
            reader,
        ));
    }
    write_entries(&root, &rows);
    assert_eq!(run(&root).unwrap_err().code, "FMINE-REF-104");
    let root = workspace("disagreed-guidance", "50%", "");
    let rows = guidance_entries("Guidance.pdf", "attributable:1/2").replacen(
        "token = \"100 - 120\"",
        "token = \"101 - 121\"",
        1,
    );
    write_entries(&root, &rows);
    assert_eq!(run(&root).unwrap_err().code, "FMINE-REF-202");
}

#[test]
fn copper_completion_cli_renders_truthful_units_and_keeps_technical_limits_unavailable() {
    let root = workspace("cli", "50%", "");
    let (envelope, _) = fmine_cli::invoke(&arguments(), &root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert!(
        envelope
            .payload_json
            .contains("\"kind\":\"sold_million_lb\"")
    );
    assert!(!envelope.payload_json.contains("_koz") && !envelope.payload_json.contains("_per_oz"));
    let mut markdown = arguments();
    markdown.extend(["--format".into(), "markdown".into()]);
    let (text, exit) = fmine_cli::run(&markdown, &root);
    assert_eq!(exit, 0, "{text}");
    for expected in [
        "kt",
        "million lb",
        "USD per sold lb",
        "239.680025",
        "320.337475",
        "T3",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(!text.contains("koz") && !text.contains("USD per sold oz"));
    // Even supplied gold technical evidence cannot turn a copper requirement into an ounce calculation.
    register_document(&root, "Technical.pdf", "2030-06-30", "technical_report");
    let mut rows = guidance_entries("Guidance.pdf", "attributable:1/2");
    for (suffix, token, unit) in [
        ("remaining_dry_feed_mt", "10", "dry_mt"),
        ("head_grade_g_per_t", "2", "g_per_dry_t"),
        ("recovery_percent", "90", "percent"),
    ] {
        for reader in ["reader-a", "reader-b"] {
            rows.push_str(
                &entry(
                    "Technical.pdf",
                    suffix,
                    token,
                    unit,
                    "100%;scenario=declared_upper_bound",
                    reader,
                )
                .replace("guidance_copper_mine_", "technical_copper_mine_")
                .replace("2030FY", "2030H2"),
            );
        }
    }
    write_entries(&root, &rows);
    assert!(matches!(
        run(&root).unwrap().technical,
        TechnicalAssessment::Indeterminate { .. }
    ));
    let mut explicit = arguments();
    explicit.extend(["--capacity-document".into(), "Technical.pdf".into()]);
    let (refused, _) = fmine_cli::invoke(&explicit, &root);
    assert_eq!(refused.exit_code, 3, "{}", refused.to_json());
    assert!(refused.to_json().contains("FMINE-REF-202"));
}

#[test]
fn copper_technical_scenarios_and_explicit_limits_preserve_exact_units_and_sources() {
    for (share, basis, maximum, grade_denominator) in [
        ("50%", "attributable:1/2", 90, 900_000),
        ("100%", "100%", 180, 1_800_000),
    ] {
        let root = workspace("technical-positive", share, "");
        let arithmetic = run(&root).unwrap();
        register_document(&root, "Technical.pdf", "2030-07-01", "technical_report");
        let rows = format!(
            "{}{}{}",
            guidance_entries("Guidance.pdf", basis),
            technical_entries("Technical.pdf", "declared_upper_bound", false),
            technical_entries("Technical.pdf", "declared_upper_bound", true),
        );
        write_entries(&root, &rows);
        let report = run(&root).unwrap();
        assert_eq!(report.completion, arithmetic.completion);
        assert_eq!(report.history, arithmetic.history);
        let c = technical_comparison(&report);
        assert_eq!(c.kind, TechnicalScenarioKind::DeclaredUpperBound);
        assert_eq!(c.result.verdict, CapacityVerdict::NotExcluded);
        // Independent dimensional answer: 10 Mt * (2/100) * (90/100) * 1000 kt/Mt * share.
        assert_eq!(
            c.result.maximum_production.value.kind,
            QuantityKind::ProductionKt
        );
        assert_eq!(
            c.result.maximum_production.value.interval,
            interval((maximum, 1), (maximum, 1))
        );
        assert_eq!(
            c.result.optimistic_required_grade.value.interval,
            interval((11999, grade_denominator), (16001, grade_denominator))
        );
        assert_eq!(
            c.limits.feed.value.kind,
            PhysicalKind::RemainingCopperDryFeedMt
        );
        assert_eq!(
            c.limits.grade.value.kind,
            PhysicalKind::CopperHeadGradeFraction
        );
        assert_eq!(
            c.limits.recovery.value.kind,
            PhysicalKind::CopperRecoveryFraction
        );
        assert_eq!(c.limits.grade.value.interval, interval((1, 50), (1, 50)));
        assert_eq!(c.limits.recovery.value.interval, interval((9, 10), (9, 10)));
        assert_eq!(c.sources.len(), 3);
        for input in [&c.limits.feed, &c.limits.grade, &c.limits.recovery] {
            input.check_id().unwrap();
            assert!(
                c.result
                    .maximum_production
                    .parents
                    .contains(&input.claim_id)
            );
            assert_eq!(
                input.anchors[0].artifact_sha256,
                sha256(&document("Technical.pdf")).0
            );
        }
        let json = report.to_json();
        assert!(json.contains("\"copper_head_grade_fraction_of_dry_mass\""));
        assert!(json.contains("\"physical_impossibility_claim\":false"));
        assert!(!json.contains("_koz") && !json.contains("gold_head_grade"));
        assert_eq!(json, run(&root).unwrap().to_json());

        let mut args = arguments();
        args.extend(["--capacity-document".into(), "Technical.pdf".into()]);
        let (envelope, _) = fmine_cli::invoke(&args, &root);
        assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
        let json = envelope.to_json();
        assert!(json.contains("\"scope\":\"copper_production_ceiling_only\""));
        assert!(json.contains("\"unit\":\"kt\""));
        assert!(json.contains("\"unit\":\"copper_fraction_dry_mass\""));
        assert!(json.contains("\"bounds_unit\":\"copper_fraction_dry_mass\""));
        assert!(json.contains("\"bounds_unit\":\"fraction\""));
        assert!(json.contains("\"bounds_conversion\":\"source percent divided by 100\""));
        assert!(json.contains(&format!("\"num\":\"{maximum}\",\"den\":\"1\"")));
        assert!(!json.contains("koz") && !json.contains("gold-production"));
        assert_eq!(json, fmine_cli::invoke(&args, &root).0.to_json());
        args.extend(["--format".into(), "markdown".into()]);
        let (markdown, exit) = fmine_cli::run(&args, &root);
        assert_eq!(exit, 0, "{markdown}");
        for expected in [
            "T2",
            "copper mass fraction of dry feed",
            "percent",
            "kt",
            "USD per sold lb",
        ] {
            assert!(
                markdown.contains(expected),
                "missing {expected}: {markdown}"
            );
        }
        assert!(!markdown.contains("koz") && !markdown.contains("g per dry"));
    }
}

#[test]
fn copper_plan_rounding_and_strict_source_admission_keep_indeterminate_distinct_from_excluded() {
    let root = workspace("technical-admission", "50%", "");
    register_document(&root, "Technical.pdf", "2030-07-01", "technical_report");
    let guidance = guidance_entries("Guidance.pdf", "attributable:1/2");
    let limits = technical_entries("Technical.pdf", "declared_upper_bound", false);
    write_entries(&root, &format!("{guidance}{limits}"));
    let original = run(&root).unwrap();
    for (scenario, expected_kind) in [
        ("plan", TechnicalScenarioKind::Plan),
        ("nameplate", TechnicalScenarioKind::Nameplate),
    ] {
        write_entries(
            &root,
            &format!(
                "{guidance}{}",
                technical_entries("Technical.pdf", scenario, false)
            ),
        );
        let rounded = run(&root).unwrap();
        let c = technical_comparison(&rounded);
        assert_eq!(c.kind, expected_kind);
        assert_eq!(c.limits.feed.state, KnowledgeState::Estimated);
        assert_eq!(c.limits.grade.value.interval, interval((3, 200), (1, 40)));
        assert_eq!(
            c.limits.recovery.value.interval,
            interval((179, 200), (181, 200))
        );
        // 10.5 * 0.025 * 0.905 * 1000 * 1/2 = 118.78125 kt, not 2.5 or 90.5 as fractions.
        assert_eq!(
            c.result.maximum_production.value.interval.hi,
            Rat::new(3801, 32).unwrap()
        );
        assert_eq!(rounded.completion, original.completion);
        assert_eq!(rounded.history, original.history);
    }
    let excluded = limits.replace("token = \"10\"", "token = \"1\"");
    write_entries(&root, &format!("{guidance}{excluded}"));
    let excluded = run(&root).unwrap();
    assert_eq!(
        technical_comparison(&excluded).result.verdict,
        CapacityVerdict::ExceedsStatedProductionCeiling
    );
    assert_eq!(excluded.completion, original.completion);
    for (old, new, code) in [
        (";metal=copper", "", "FMINE-REF-105"),
        ("percent_dry_mass", "percent", "FMINE-REF-105"),
        ("dry_mt", "wet_mt", "FMINE-REF-105"),
        ("2030H2", "2030FY", "FMINE-REF-102"),
        ("token = \"2\"", "token = \"101\"", "FMINE-REF-109"),
        ("token = \"90\"", "token = \"101\"", "FMINE-REF-109"),
    ] {
        write_entries(&root, &format!("{guidance}{}", limits.replace(old, new)));
        let bad = run(&root).unwrap();
        assert_eq!(technical_refusal(&bad).code, code, "{old} -> {new}");
        assert_eq!(bad.completion, original.completion);
        assert_eq!(bad.history, original.history);
    }
    register_document(&root, "Updated.pdf", "2030-07-20", "technical_report");
    let pending = entry(
        "Updated.pdf",
        "head_grade_percent",
        "1.5",
        "percent_dry_mass",
        "100%;scenario=plan;metal=copper",
        "reader-a",
    )
    .replace("guidance_copper_mine_", "technical_copper_mine_")
    .replace("2030FY", "2030H2");
    write_entries(&root, &format!("{guidance}{limits}{pending}"));
    let pending = run(&root).unwrap();
    assert_eq!(technical_refusal(&pending).code, "FMINE-REF-202");
    assert!(technical_refusal(&pending).what.contains("Updated.pdf"));
    assert_eq!(pending.completion, original.completion);
    assert_eq!(pending.history, original.history);
    // A newer agreed gold-grade-only row is still a latest technical document for this site.
    // It cannot make the selector resurrect the older complete copper limits.
    let mut wrong_family = String::new();
    for reader in ["reader-a", "reader-b"] {
        wrong_family.push_str(
            &entry(
                "Updated.pdf",
                "head_grade_g_per_t",
                "2",
                "g_per_dry_t",
                "100%;scenario=plan",
                reader,
            )
            .replace("guidance_copper_mine_", "technical_copper_mine_")
            .replace("2030FY", "2030H2"),
        );
    }
    write_entries(&root, &format!("{guidance}{limits}{wrong_family}"));
    let wrong_family = run(&root).unwrap();
    assert_eq!(technical_refusal(&wrong_family).code, "FMINE-REF-202");
    assert!(
        technical_refusal(&wrong_family)
            .what
            .contains("Updated.pdf")
    );
    assert_eq!(wrong_family.completion, original.completion);
    assert_eq!(wrong_family.history, original.history);
}

#[test]
fn explicit_copper_limits_reject_wrong_metal_unit_period_and_changed_source_bytes() {
    let root = workspace("explicit-copper-admission", "50%", "");
    register_document(&root, "Technical.pdf", "2030-07-01", "technical_report");
    let guidance = guidance_entries("Guidance.pdf", "attributable:1/2");
    let limits = technical_entries("Technical.pdf", "declared_upper_bound", true);
    let mut args = arguments();
    args.extend(["--capacity-document".into(), "Technical.pdf".into()]);
    for (old, new, code) in [
        (";metal=copper", ";metal=gold", "FMINE-REF-107"),
        ("percent_dry_mass", "fraction", "FMINE-REF-105"),
        ("unit = \"percent\"", "unit = \"fraction\"", "FMINE-REF-105"),
        ("2030-07/2030-12", "2030FY", "FMINE-REF-102"),
        (
            "Explicit maximum limits",
            "Nameplate estimate",
            "FMINE-REF-107",
        ),
        ("token = \"90\"", "token = \"101\"", "FMINE-REF-109"),
    ] {
        write_entries(&root, &format!("{guidance}{}", limits.replace(old, new)));
        let (envelope, _) = fmine_cli::invoke(&args, &root);
        assert_ne!(envelope.exit_code, 0);
        assert!(
            envelope.to_json().contains(code),
            "{old} -> {new}: {}",
            envelope.to_json()
        );
    }
    write_entries(&root, &format!("{guidance}{limits}"));
    assert_eq!(fmine_cli::invoke(&args, &root).0.exit_code, 0);
    std::fs::write(
        root.join("corpus/Technical.pdf"),
        b"SYNTHETIC altered technical-report bytes",
    )
    .unwrap();
    let (envelope, _) = fmine_cli::invoke(&args, &root);
    assert_ne!(envelope.exit_code, 0);
    assert!(
        envelope.to_json().contains("FMINE-REF-505"),
        "{}",
        envelope.to_json()
    );
}
