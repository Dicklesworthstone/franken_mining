use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_public::sources::{registry_path, transcriptions_path};

const DOCUMENT: &[u8] =
    b"%PDF-1.7 invented cut-off assumptions; not a parsed or real technical report";
const UNRELATED: &[u8] =
    b"%PDF-1.7 unrelated invented technical report, intentionally absent on disk";
const BASIS: &str = "full;dry;resource;gold";
const PRICE_BASIS: &str = "resource_price_usd_per_gold_oz";
const PERIOD: &str = "effective_2024_12_31";
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Model {
    Constant,
    Tcm,
    ShortTon,
}

impl Model {
    fn id(self) -> &'static str {
        match self {
            Self::Constant => "constant_cutoff",
            Self::Tcm => "tcm_cutoff",
            Self::ShortTon => "short_ton_cutoff",
        }
    }

    fn values(self) -> Vec<(&'static str, &'static str, &'static str)> {
        let mut rows = vec![
            ("gold_price", "3,110.34768", "usd_per_oz"),
            ("cost", "100.0", "usd_per_t"),
        ];
        match self {
            Self::Constant => rows.extend([
                ("published", "2.00", "g_per_t"),
                ("recovery", "50.0", "percent"),
            ]),
            Self::Tcm => rows.extend([
                ("published", "1.50", "g_per_t"),
                ("recovery_intercept", "80.0", "percent"),
                ("recovery_slope", "2.00", "g_per_t"),
                ("tcm", "10.0", "percent"),
            ]),
            Self::ShortTon => rows.extend([
                ("published", "0.058", "oz_per_short_ton"),
                ("published_metric", "2.0", "g_per_t"),
                ("recovery", "50.0", "percent"),
            ]),
        }
        rows
    }
}

#[derive(Clone)]
struct Entry {
    quantity: String,
    row: String,
    token: String,
    unit: String,
    basis: String,
    period: String,
    reader: &'static str,
}

struct Fixture {
    root: PathBuf,
    definitions: String,
    entries: Vec<Entry>,
}

impl Fixture {
    fn new(tag: &str, models: &[Model]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-cutoff-contract-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        for dir in [
            "documents",
            "registries/sources",
            "registries/transcriptions",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("documents/Mine.pdf"), DOCUMENT).unwrap();
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"documents\"\n\
             retrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        for (file, bytes) in [("Mine.pdf", DOCUMENT), ("Unrelated.pdf", UNRELATED)] {
            write!(registry,
                "\n[[document]]\nfile = \"{file}\"\nkind = \"technical_report\"\n\
                 url = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\n\
                 published_at = \"2025-03-01\"\npublished_at_basis = \"FACT: invented fixture publication date\"\n\
                 pages = 10\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(), sha256(bytes).to_hex()).unwrap();
        }
        registry.push_str("\n[[subject]]\nname = \"Invented Mine\"\n");
        std::fs::write(registry_path(&root, "synthetic"), registry).unwrap();
        let mut definitions =
            String::from("schema = \"fmine.table-identities.v1\"\nissuer = \"synthetic\"\n");
        let mut entries = Vec::new();
        for model in models {
            let id = model.id();
            let unit = if matches!(model, Model::ShortTon) {
                "oz_per_short_ton"
            } else {
                "g_per_t"
            };
            let recovery = if matches!(model, Model::Tcm) {
                "tcm_over_gold"
            } else {
                "constant_percent"
            };
            write!(
                definitions,
                "\n[[cutoff]]\nid = \"{id}\"\ndoc = \"Mine.pdf\"\nsubject = \"Invented Mine\"\n\
                 period = \"{PERIOD}\"\nbasis = \"{BASIS}\"\nprice_basis = \"{PRICE_BASIS}\"\n\
                 unit = \"{unit}\"\nrecovery = \"{recovery}\"\n"
            )
            .unwrap();
            for (role, token, unit) in model.values() {
                let quantity = format!("{id}_{role}");
                write!(definitions,
                    "\n[[cutoff_input]]\ncutoff = \"{id}\"\nrole = \"{role}\"\nquantity = \"{quantity}\"\n").unwrap();
                for reader in ["synthetic_reader_a", "synthetic_reader_b"] {
                    entries.push(Entry {
                        row: quantity.clone(),
                        quantity: quantity.clone(),
                        token: token.into(),
                        unit: unit.into(),
                        basis: if role == "gold_price" {
                            PRICE_BASIS
                        } else {
                            BASIS
                        }
                        .into(),
                        period: PERIOD.into(),
                        reader,
                    });
                }
            }
        }
        let fixture = Self {
            root,
            definitions,
            entries,
        };
        fixture.write();
        fixture
    }

    fn write(&self) {
        std::fs::write(self.root.join("cutoffs.toml"), &self.definitions).unwrap();
        let mut readings =
            String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
        for entry in &self.entries {
            write!(
                readings,
                "\n[[entry]]\ndoc = \"Mine.pdf\"\ndoc_sha256 = \"{}\"\npage = 2\n\
                 table = \"Invented cut-off assumptions\"\nrow = \"{}\"\ncol = \"Declared\"\n\
                 token = \"{}\"\nquantity = \"{}\"\nunit = \"{}\"\nbasis = \"{}\"\n\
                 period = \"{}\"\nreader = \"{}\"\n",
                sha256(DOCUMENT).to_hex(),
                entry.row,
                entry.token,
                entry.quantity,
                entry.unit,
                entry.basis,
                entry.period,
                entry.reader
            )
            .unwrap();
        }
        std::fs::write(transcriptions_path(&self.root, "synthetic"), readings).unwrap();
    }

    fn change(&mut self, quantity: &str, edit: impl Fn(&mut Entry)) {
        let mut count = 0;
        for entry in &mut self.entries {
            if entry.quantity == quantity {
                edit(entry);
                count += 1;
            }
        }
        assert_eq!(
            count, 2,
            "mutation must cover both independent synthetic readings"
        );
        self.write();
    }

    fn invoke(&self, extra: &[&str]) -> (i32, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args([
                "check",
                "tables",
                "--issuer",
                "synthetic",
                "--definitions",
                "cutoffs.toml",
            ])
            .args(extra)
            .arg("--workspace")
            .arg(&self.root)
            .output()
            .expect("execute actual fmine binary");
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        (
            output.status.code().expect("exit code"),
            String::from_utf8(output.stdout).unwrap(),
        )
    }

    fn successful(&self, extra: &[&str]) -> String {
        let (exit, output) = self.invoke(extra);
        assert_eq!(exit, 0, "{output}");
        output
    }
}

// Locate a complete nested JSON object without depending on a production serializer's whitespace.
fn object<'a>(json: &'a str, prefix: &str) -> &'a str {
    let start = json
        .find(prefix)
        .unwrap_or_else(|| panic!("missing {prefix:?}: {json}"));
    assert!(json[start..].starts_with('{'));
    let (mut depth, mut quoted, mut escaped) = (0_u32, false, false);
    for (offset, byte) in json[start..].bytes().enumerate() {
        if escaped {
            escaped = false;
        } else if quoted && byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            quoted = !quoted;
        } else if !quoted {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &json[start..=start + offset];
                    }
                }
                _ => {}
            }
        }
    }
    panic!("unterminated object: {prefix}");
}

fn cutoff_report(json: &str) -> &str {
    object(json, "{\"schema\":\"fmine.source-cutoffs.v1\"")
}

fn cutoff_row<'a>(json: &'a str, id: &str) -> &'a str {
    object(
        cutoff_report(json),
        &format!("{{\"id\":\"{id}\",\"declaration\":"),
    )
}

fn assert_row_refused(json: &str, id: &str, why: &str) {
    let row = cutoff_row(json, id);
    assert!(
        row.contains("\"status\":\"refused\",\"result\":null"),
        "{row}"
    );
    assert!(row.contains(why), "{row}");
    assert!(
        row.contains("\"claims\":null"),
        "refused rows must not emit Claims: {row}"
    );
}

fn string_field<'a>(json: &'a str, key: &str) -> &'a str {
    json.split_once(&format!("\"{key}\":\""))
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
}

fn claim_objects(row: &str) -> (Vec<&str>, &str) {
    let claims = row.split_once("\"claims\":").unwrap().1;
    let mut rest = claims.strip_prefix("{\"inputs\":[").unwrap();
    let mut inputs = Vec::new();
    while rest.starts_with('{') {
        let input = object(rest, "{");
        inputs.push(input);
        rest = rest[input.len()..].trim_start_matches(',');
    }
    (
        inputs,
        object(rest.split_once("\"derived\":").unwrap().1, "{"),
    )
}

fn assert_canonical_id(claim: &str) {
    let hex = string_field(claim, "canonical_body");
    assert_eq!(hex.len() % 2, 0);
    let bytes: Vec<_> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect();
    assert_eq!(
        string_field(claim, "claim_id"),
        ObjectId::from_canonical_bytes(PrefixKind::Claim, &bytes).to_string(),
    );
    assert!(claim.contains("\"evidence\":\"issuer_assertion\""));
    assert!(claim.contains("\"confidentiality\":\"public\""));
    assert!(claim.contains("\"representation\":\"deterministic\""));
}

#[test]
fn cutoff_cli_reproduces_constant_tcm_and_exact_short_ton_conversion() {
    let fixture = Fixture::new(
        "three-models",
        &[Model::Constant, Model::Tcm, Model::ShortTon],
    );
    let output = fixture.successful(&["--json"]);
    let constant = cutoff_row(&output, "constant_cutoff");
    assert!(constant.contains("\"status\":\"reproduced\""), "{constant}");
    // Hand result: c=100*31.1034768/3110.34768=1; g=c/0.5=2.
    // Bounds: (100+-0.05)/100 divided by (50-+0.05)/100.
    assert!(constant.contains("\"exact\":{\"published\":\"2/1\",\"computed\":\"2/1\",\"low\":\"1999/1001\",\"high\":\"667/333\"}"), "{constant}");
    let tcm = cutoff_row(&output, "tcm_cutoff");
    // R%=80-2*10/g; (1 + 2*10/100)/(80/100)=3/2.
    assert!(tcm.contains("\"computed\":\"3/2\""), "{tcm}");
    assert!(tcm.contains("\"status\":\"reproduced\""));
    assert!(!tcm.contains("\"solver_iterations\":null"));
    let us = cutoff_row(&output, "short_ton_cutoff");
    assert!(us.contains("\"unit\":\"oz/st\""));
    assert!(us.contains("\"computed\":\"7/120\""), "{us}");
    assert!(us.contains("\"metric_from_rounded\":true"));
    assert!(output.contains("\"source_bytes_verified\":true"));
    assert!(output.contains("\"as_of\":null"));
    assert!(output.contains("\"anchor_id\":null"));
    assert!(output.contains("\"certificate\":null"));
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(output.contains(&format!("\"{flag}\":false")));
    }
    assert!(constant.contains(&sha256(DOCUMENT).to_hex()));
    assert!(constant.contains("\"token\":\"3,110.34768\""));
    assert!(constant.contains("\"readers\":[\"synthetic_reader_a\",\"synthetic_reader_b\"]"));
    assert!(
        constant.contains("\"published_at_basis\":\"FACT: invented fixture publication date\"")
    );
}

#[test]
fn cutoff_cli_library_binary_json_and_markdown_agree() {
    let fixture = Fixture::new("rendering", &[Model::Constant]);
    let args = [
        "check",
        "tables",
        "--issuer",
        "synthetic",
        "--definitions",
        "cutoffs.toml",
    ]
    .map(str::to_owned);
    let (envelope, _) = fmine_cli::invoke(&args, &fixture.root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(
        fixture.successful(&["--json"]),
        format!("{}\n", envelope.to_json())
    );
    let markdown = fixture.successful(&["--format", "markdown"]);
    assert_eq!(
        markdown,
        format!("{}\n", envelope.rendered_markdown.unwrap())
    );
    assert!(markdown.contains("Source-bound published cut-offs"));
    assert!(markdown.contains("computed 2/1"));
    assert!(markdown.contains("constant_cutoff_cost"));
    assert!(markdown.contains(&sha256(DOCUMENT).to_hex()));
}

#[test]
fn cutoff_cli_requires_selected_document_bytes_but_scopes_unrelated_documents() {
    let fixture = Fixture::new("bytes", &[Model::Constant]);
    assert!(!fixture.root.join("documents/Unrelated.pdf").exists());
    fixture.successful(&[]);
    std::fs::write(
        fixture.root.join("documents/Mine.pdf"),
        b"altered invented source",
    )
    .unwrap();
    let (exit, output) = fixture.invoke(&[]);
    assert_eq!(exit, 8, "{output}");
    assert!(output.contains("FMINE-REF-505"));
    assert!(output.contains("Mine.pdf"));
    let path = registry_path(&fixture.root, "synthetic");
    let registry = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        registry.replace(
            "corpus_dir = \"documents\"",
            "corpus_dir = \"missing_documents\"",
        ),
    )
    .unwrap();
    let (exit, output) = fixture.invoke(&[]);
    assert_eq!(exit, 5, "{output}");
    assert!(output.contains("FMINE-REF-506"));
    assert!(output.contains("Mine.pdf"));
}

#[test]
fn cutoff_cli_as_of_retains_population_and_refuses_unavailable_sources() {
    let fixture = Fixture::new("as-of", &[Model::Constant, Model::Tcm]);
    let (exit, output) = fixture.invoke(&["--as-of", "2025-02-28"]);
    assert_eq!(exit, 3, "{output}");
    assert!(output.contains("FMINE-REF-202"));
    assert!(output.contains("Mine.pdf"));
    let admitted = fixture.successful(&["--as-of", "2025-03-01"]);
    assert!(admitted.contains("\"as_of\":\"2025-03-01\""));
    assert!(cutoff_row(&admitted, "constant_cutoff").contains("\"status\":\"reproduced\""));
    assert!(cutoff_row(&admitted, "tcm_cutoff").contains("\"status\":\"reproduced\""));
    let (exit, _) = fixture.invoke(&["--as-of", "2025-02-30"]);
    assert_ne!(exit, 0);
}

#[test]
fn cutoff_cli_as_of_requires_a_date_and_pdf_definitions() {
    let fixture = Fixture::new("invalid-cut", &[Model::Constant]);
    let (exit, output) = fixture.invoke(&["--as-of", "2025-03-01T00:00:00Z"]);
    assert_eq!(exit, 2, "{output}");
    assert!(
        output.contains("check tables --as-of requires --definitions and YYYY-MM-DD"),
        "{output}"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args([
            "check",
            "tables",
            "--issuer",
            "synthetic",
            "--as-of",
            "2025-03-01",
            "--json",
            "--workspace",
        ])
        .arg(&fixture.root)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("--definitions")
    );
    let (exit, output) = fixture.invoke(&["--through", "2024Q4", "--as-of", "2025-03-01"]);
    assert_eq!(exit, 2, "{output}");
    assert!(
        output
            .contains("--definitions selects PDF identities; --through selects workbook releases"),
        "{output}"
    );
}

#[test]
fn cutoff_cli_keeps_single_reader_and_missing_inputs_as_refused_rows() {
    let mut fixture = Fixture::new("pending", &[Model::Constant, Model::Tcm]);
    fixture
        .entries
        .retain(|e| !(e.quantity == "constant_cutoff_cost" && e.reader == "synthetic_reader_b"));
    fixture.write();
    let pending = fixture.successful(&[]);
    assert_row_refused(
        &pending,
        "constant_cutoff",
        "no agreed reading for quantity constant_cutoff_cost",
    );
    assert!(pending.contains("\"pending_readings\":[{\"locator\":\"Mine.pdf:p2:Invented cut-off assumptions:constant_cutoff_cost/Declared\""));
    assert!(cutoff_row(&pending, "tcm_cutoff").contains("\"status\":\"reproduced\""));
    fixture
        .entries
        .retain(|e| e.quantity != "constant_cutoff_cost");
    fixture.write();
    let missing = fixture.successful(&[]);
    assert_row_refused(
        &missing,
        "constant_cutoff",
        "no agreed reading for quantity constant_cutoff_cost",
    );
    assert!(missing.contains("\"pending_readings\":[]"));
    assert_ne!(pending, missing);
}

#[test]
fn cutoff_cli_cannot_scope_away_a_conflicting_readers_quantity() {
    for (tag, change_quantity) in [("disagree-value", false), ("disagree-key", true)] {
        let mut fixture = Fixture::new(tag, &[Model::Constant]);
        let entry = fixture
            .entries
            .iter_mut()
            .find(|e| e.quantity == "constant_cutoff_cost" && e.reader == "synthetic_reader_b")
            .unwrap();
        if change_quantity {
            entry.quantity = "unselected_cost".into();
        } else {
            entry.token = "101.0".into();
        }
        fixture.write();
        let (exit, output) = fixture.invoke(&[]);
        assert_eq!(exit, 3, "{output}");
        assert!(output.contains("FMINE-REF-202"));
        assert!(output.contains("readers disagree"));
        assert!(output.contains("constant_cutoff_cost"));
    }
}

#[test]
fn cutoff_cli_refuses_unit_basis_period_and_range_substitutions() {
    for mutation in ["unit", "basis", "period", "range"] {
        let mut fixture = Fixture::new(mutation, &[Model::Constant]);
        fixture.change("constant_cutoff_cost", |e| match mutation {
            "unit" => e.unit = "usd_per_short_ton".into(),
            "basis" => e.basis = "attributable;wet;reserve;gold".into(),
            "period" => e.period = "effective_2023_12_31".into(),
            "range" => e.token = "99.0 - 101.0".into(),
            _ => unreachable!(),
        });
        let output = fixture.successful(&[]);
        assert_row_refused(
            &output,
            "constant_cutoff",
            if mutation == "range" {
                "printed point"
            } else {
                "unit, basis or period differs"
            },
        );
    }
}

#[test]
fn cutoff_cli_rejects_duplicate_missing_and_literal_definition_inputs() {
    for mutation in ["duplicate", "missing", "literal"] {
        let mut fixture = Fixture::new(mutation, &[Model::Constant]);
        fixture.definitions = match mutation {
            "duplicate" => fixture.definitions.replace(
                "quantity = \"constant_cutoff_cost\"",
                "quantity = \"constant_cutoff_gold_price\"",
            ),
            "missing" => fixture
                .definitions
                .replace("role = \"recovery\"", "role = \"recovery_intercept\""),
            "literal" => fixture.definitions.replace(
                "unit = \"g_per_t\"",
                "unit = \"g_per_t\"\npublished = \"2.00\"",
            ),
            _ => unreachable!(),
        };
        fixture.write();
        let (exit, output) = fixture.invoke(&[]);
        assert_eq!(exit, 3, "{output}");
        assert!(output.contains("FMINE-REF-202"));
    }
}

#[test]
fn cutoff_cli_retains_conflicting_published_value_and_moves_digest_on_source_change() {
    let mut fixture = Fixture::new("conflict", &[Model::Constant]);
    let before = fixture.successful(&[]);
    fixture.change("constant_cutoff_cost", |e| e.token = "200.0".into());
    let changed = fixture.successful(&[]);
    let row = cutoff_row(&changed, "constant_cutoff");
    assert!(row.contains("\"status\":\"conflict\""), "{row}");
    assert!(row.contains("\"published\":\"2/1\",\"computed\":\"4/1\""));
    assert!(row.contains("\"token\":\"2.00\""));
    assert!(row.contains("\"token\":\"200.0\""));
    assert_ne!(cutoff_report(&before), cutoff_report(&changed));
}

#[test]
fn cutoff_cli_order_is_canonical_and_declared_scope_does_not_read_unused_values() {
    let mut fixture = Fixture::new("order", &[Model::Constant]);
    let before = fixture.successful(&[]);
    fixture.entries.reverse();
    fixture.write();
    assert_eq!(before, fixture.successful(&[]));
    let mut blocks = fixture.definitions.split("[[cutoff_input]]");
    let prefix = blocks.next().unwrap().to_string();
    let mut inputs: Vec<_> = blocks.map(str::to_owned).collect();
    inputs.reverse();
    fixture.definitions = prefix
        + &inputs
            .iter()
            .map(|b| format!("[[cutoff_input]]{b}"))
            .collect::<String>();
    fixture.write();
    let reordered = fixture.successful(&[]);
    assert_eq!(cutoff_report(&before), cutoff_report(&reordered));
    assert_ne!(
        before, reordered,
        "literal definitions digest retains file changes"
    );
    let mut unrelated = fixture.entries[0].clone();
    unrelated.quantity = "unselected_assumption".into();
    unrelated.row = "unselected_assumption".into();
    unrelated.token = "777.0".into();
    fixture.entries.push(unrelated);
    fixture.write();
    assert_eq!(
        reordered,
        fixture.successful(&[]),
        "one unrelated reader cannot contaminate declared quantities"
    );
}

#[test]
fn cutoff_cli_claim_ids_reconstruct_and_derived_parents_preserve_every_source() {
    let fixture = Fixture::new("claim-ancestry", &[Model::Constant]);
    let output = fixture.successful(&[]);
    let (inputs, derived) = claim_objects(cutoff_row(&output, "constant_cutoff"));
    assert_eq!(inputs.len(), 4);
    let mut parent_ids = Vec::new();
    let mut source_anchors = Vec::new();
    for input in inputs {
        assert_canonical_id(input);
        assert!(input.contains("\"parents\":[]"));
        parent_ids.push(format!("\"{}\"", string_field(input, "claim_id")));
        let anchor = object(input.split_once("\"anchors\":[").unwrap().1, "{");
        assert_eq!(string_field(anchor, "artifact_path"), "documents/Mine.pdf");
        assert_eq!(
            string_field(anchor, "artifact_sha256"),
            sha256(DOCUMENT).to_hex()
        );
        assert_eq!(
            string_field(anchor, "locator"),
            format!(
                "Mine.pdf:p2:Invented cut-off assumptions:{}/Declared",
                string_field(input, "quantity")
            ),
        );
        source_anchors.push(anchor);
    }
    assert_canonical_id(derived);
    parent_ids.sort();
    assert!(parent_ids.windows(2).all(|pair| pair[0] != pair[1]));
    source_anchors.sort();
    source_anchors.dedup();
    assert_eq!(source_anchors.len(), 4);
    assert!(derived.contains(&format!("\"parents\":[{}]", parent_ids.join(","))));
    assert!(derived.contains(&format!("\"anchors\":[{}]", source_anchors.join(","))));
    assert!(derived.contains("\"metric_key\":\"cutoff.reproduction\""));
    assert!(derived.contains("\"computed\":\"2/1\""));
}

#[test]
fn cutoff_cli_precision_changes_source_and_derived_identity_without_changing_point() {
    let mut fixture = Fixture::new("precision-identity", &[Model::Constant]);
    let before = fixture.successful(&[]);
    fixture.change("constant_cutoff_cost", |e| e.token = "100.00".into());
    let after = fixture.successful(&[]);
    let (old_inputs, old_derived) = claim_objects(cutoff_row(&before, "constant_cutoff"));
    let (new_inputs, new_derived) = claim_objects(cutoff_row(&after, "constant_cutoff"));
    let old_cost = old_inputs
        .into_iter()
        .find(|c| string_field(c, "role") == "cost")
        .unwrap();
    let new_cost = new_inputs
        .into_iter()
        .find(|c| string_field(c, "role") == "cost")
        .unwrap();
    assert_eq!(string_field(old_cost, "printed"), "100/1");
    assert_eq!(string_field(new_cost, "printed"), "100/1");
    assert_eq!(string_field(old_cost, "half_step"), "1/20");
    assert_eq!(string_field(new_cost, "half_step"), "1/200");
    assert_ne!(
        string_field(old_cost, "claim_id"),
        string_field(new_cost, "claim_id")
    );
    assert_ne!(
        string_field(old_derived, "claim_id"),
        string_field(new_derived, "claim_id")
    );
    assert!(old_derived.contains("\"computed\":\"2/1\""));
    assert!(new_derived.contains("\"computed\":\"2/1\""));
    assert_canonical_id(new_cost);
    assert_canonical_id(new_derived);
    assert_ne!(cutoff_report(&before), cutoff_report(&after));
}

#[test]
fn cutoff_cli_combines_separate_cost_revenue_and_per_ounce_source_claims() {
    let mut fixture = Fixture::new("cost-deductions", &[Model::Constant]);
    for (role, quantity, token, unit) in [
        ("cost", "additional_cost", "50.0", "usd_per_t"),
        ("revenue_deduction", "royalty_percent", "10.0", "percent"),
        ("ounce_deduction", "ounce_fee", "31.1034768", "usd_per_oz"),
    ] {
        write!(fixture.definitions,
            "\n[[cutoff_input]]\ncutoff = \"constant_cutoff\"\nrole = \"{role}\"\nquantity = \"{quantity}\"\n").unwrap();
        for reader in ["synthetic_reader_a", "synthetic_reader_b"] {
            fixture.entries.push(Entry {
                quantity: quantity.into(),
                row: quantity.into(),
                token: token.into(),
                unit: unit.into(),
                basis: BASIS.into(),
                period: PERIOD.into(),
                reader,
            });
        }
    }
    fixture.change("constant_cutoff_published", |e| e.token = "3.37".into());
    let output = fixture.successful(&[]);
    let row = cutoff_row(&output, "constant_cutoff");
    // Net price is (0.90 - 0.01)*3110.34768; total cost 150 and recovery 0.5 give 300/89.
    assert!(row.contains("\"status\":\"reproduced\""), "{row}");
    assert!(row.contains("\"computed\":\"300/89\""), "{row}");
    let (inputs, derived) = claim_objects(row);
    assert_eq!(inputs.len(), 7);
    for quantity in ["additional_cost", "royalty_percent", "ounce_fee"] {
        assert!(
            inputs
                .iter()
                .any(|c| string_field(c, "quantity") == quantity)
        );
    }
    assert_canonical_id(derived);
}
