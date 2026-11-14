// Shared invented filing fixtures for library and actual-binary peer comparison controls.
// Each reader row remains bound to its generated document bytes and explicit metric definition.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_metrics::comparison::Metric;

const ISSUERS: [&str; 4] = ["peer_a", "peer_b", "peer_c", "peer_d"];
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    metric: Metric,
}

impl Fixture {
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
