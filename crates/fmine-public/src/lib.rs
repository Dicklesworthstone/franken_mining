//! Public lane (`GATE-BULL-01`, `BD-055`, `BD-058`, `BD-066`, `BD-321`, `BD-322`): loads the registered public
//! workbooks of an issuer, refusing unless every file is present with its registered SHA-256; measures
//! extraction (cell census and numeric-cell digest per workbook); builds the five-state identity ledger over
//! the issuer's AISC reconciliation tables; and commits the whole result to a state root.
//!
//! Nothing here is a stand-in: a missing or altered input is a typed refusal (`FMINE-REF-502`) naming the
//! file. Synthetic workbooks exist only in the synthetic lane, which says so and is never gate-eligible.

pub mod aisc;
pub mod mine_stats;
pub mod sources;
pub mod synthetic;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::{AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport};
use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal};
use fmine_io::xlsx::{
    CellCensus, CellValue, ExtractedCell, QuarantineItem, Workbook, parse_a1_ref,
};

use crate::aisc::{
    DEFAULT_MISSING_FACT, ExplanationBasis, IdentityFamily, IdentityInstance, IdentityState,
    RateMiss, ShareBasis, UnregisteredHeader, cell_text, confirm_share_label, find_qtd_sheet,
    ledger_for_table, locate_table, reconstruction_closes,
};
use crate::mine_stats::{CellRefusal, LabelVocabulary, MineStatsRead, read_mine_stats};
pub use crate::sources::load_issuer_profile;
use crate::sources::{
    ExpectedRoot, IssuerProfile, RegisteredMissingFact, RegisteredReconstruction, SourceKind,
};

/// Converts an extracted cell into a public-basis `Claim<Option<f64>>` (`BD-058`): a number is the value as
/// published, a placeholder is a known unknown, and text or a formula error is refused.
pub fn cell_to_public_claim(
    workbook: &Workbook,
    sheet_name: &str,
    metric_key: &str,
    cell: &ExtractedCell,
    additivity: AdditivityClass,
) -> Result<Claim<Option<f64>>, Refusal> {
    let anchor = workbook.make_anchor(sheet_name, &cell.cell_ref);
    match &cell.value {
        CellValue::Number(v) => Ok(Claim::new(
            metric_key,
            Some(*v),
            &format!("{v}"),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::Exact,
            KnowledgeState::KnownKnown,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![anchor],
        )),
        CellValue::Placeholder(tok) => Ok(Claim::new(
            metric_key,
            None,
            tok,
            Representation::Placeholder { token: tok.clone() },
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::Placeholder,
            KnowledgeState::KnownUnknown,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![anchor],
        )),
        CellValue::FormulaError(err_tok) => Err(Refusal::from_code(
            "FMINE-REF-401",
            format!(
                "quarantined formula error '{err_tok}' at {sheet_name}!{}",
                cell.cell_ref
            ),
        )),
        CellValue::Text(txt) => Err(Refusal::from_code(
            "FMINE-REF-401",
            format!(
                "expected a number or placeholder at {sheet_name}!{}, found text '{txt}'",
                cell.cell_ref
            ),
        )),
    }
}

/// Which lane produced a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Registered public filings, verified by digest. Eligible for `GATE-BULL-01`.
    Public,
    /// Invented workbooks with seeded answers. Never gate-eligible.
    Synthetic,
}

impl Lane {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Synthetic => "synthetic",
        }
    }
}

/// One loaded input workbook with its provenance and extraction census.
#[derive(Debug, Clone)]
pub struct LoadedSource {
    pub file_name: String,
    pub kind: SourceKind,
    pub quarter: String,
    pub url: String,
    pub url_basis: String,
    pub bytes: u64,
    pub workbook: Workbook,
    pub census: CellCensus,
}

fn hex(bytes: &[u8; 32]) -> String {
    Digest32(*bytes).to_hex()
}

/// Loads every registered workbook of `profile` from `<root>/<corpus_dir>/`, checking byte size and SHA-256
/// against the registry. Refuses (`FMINE-REF-502`) naming every missing, altered or unparseable file.
pub fn load_registered_sources(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<Vec<LoadedSource>, Refusal> {
    let dir = root.join(&profile.corpus_dir);
    let mut loaded = Vec::with_capacity(profile.sources.len());
    let mut missing = Vec::new();
    let mut altered = Vec::new();
    let mut unreadable = Vec::new();
    for src in &profile.sources {
        let bytes = match std::fs::read(dir.join(&src.file_name)) {
            Ok(b) => b,
            Err(e) => {
                missing.push(format!("{} ({e})", src.file_name));
                continue;
            }
        };
        let digest = sha256(&bytes).to_hex();
        if bytes.len() as u64 != src.bytes || digest != src.sha256_hex {
            altered.push(format!(
                "{} (registered {} bytes sha256 {}, found {} bytes sha256 {digest})",
                src.file_name,
                src.bytes,
                src.sha256_hex,
                bytes.len()
            ));
            continue;
        }
        let rel = format!("{}/{}", profile.corpus_dir, src.file_name);
        match Workbook::from_bytes(rel, &bytes) {
            Ok(workbook) => {
                let census = workbook.cell_census();
                loaded.push(LoadedSource {
                    file_name: src.file_name.clone(),
                    kind: src.kind,
                    quarter: src.quarter.clone(),
                    url: src.url.clone(),
                    url_basis: src.url_basis.clone(),
                    bytes: src.bytes,
                    workbook,
                    census,
                });
            }
            Err(e) => unreadable.push(format!("{} ({e})", src.file_name)),
        }
    }
    if missing.is_empty() && altered.is_empty() && unreadable.is_empty() {
        return Ok(loaded);
    }
    let mut detail = format!(
        "{} of {} registered {} workbooks unusable under {}",
        missing.len() + altered.len() + unreadable.len(),
        profile.sources.len(),
        profile.issuer,
        dir.display()
    );
    for (label, list) in [
        ("missing", &missing),
        ("digest mismatch", &altered),
        ("unreadable", &unreadable),
    ] {
        if !list.is_empty() {
            let _ = write!(detail, "; {label}: {}", list.join(", "));
        }
    }
    Err(Refusal::from_code("FMINE-REF-502", detail))
}

/// A sheet that holds a cost-of-sales row but whose table structure could not be located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRefusal {
    pub file: String,
    pub sheet: String,
    pub reason: String,
}

/// Report of one assurance run (`fmine assure public`).
#[derive(Debug, Clone)]
pub struct PublicAssuranceReport {
    pub issuer: String,
    pub lane: Lane,
    pub retrieved_utc: String,
    /// The `--through YYYYQn` bound the run applied, if any. Not in the state root: the sources it kept are.
    pub through: Option<String>,
    pub sources: Vec<LoadedSource>,
    pub aisc_tables: usize,
    pub table_refusals: Vec<TableRefusal>,
    pub instances: Vec<IdentityInstance>,
    pub unregistered_headers: Vec<UnregisteredHeader>,
    /// The label-anchored read of each registered Mine Stats workbook, by file name (bd-gwf1.1). Not in the
    /// state root yet: it enters with the Mine Stats claims (BD-332.S3), so the root still moves only when an
    /// input file or the AISC ledger does.
    pub mine_stats: Vec<(String, MineStatsRead)>,
    /// The same families run over hidden worksheets (BD-331.S2). Bound into the state root in its own section
    /// and never counted in `instances`, the totals or `GATE-BULL-01`.
    pub hidden_scope: HiddenScope,
    pub ledger_anchor: LedgerAnchor,
}

/// How every hidden-scope instance is labelled.
pub const HIDDEN_SCOPE: &str = "sheet not displayed to readers";

/// The identity ledger over the AISC tables on hidden worksheets: reported, bound into the root, never counted.
#[derive(Debug, Clone, Default)]
pub struct HiddenScope {
    pub aisc_tables: usize,
    pub instances: Vec<IdentityInstance>,
    pub unregistered_headers: Vec<UnregisteredHeader>,
    pub table_refusals: Vec<TableRefusal>,
}

/// Counts of the alternative construction (subtotal rounded once), reported and never scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateCounts {
    /// Footing instances in states (i) or (iii) that the alternative construction accepts.
    pub footing_consistent: usize,
    /// Per-ounce instances consistent under the declared construction that the alternative also accepts.
    pub rate_consistent: usize,
}

impl PublicAssuranceReport {
    /// Only a public-lane run can count toward `GATE-BULL-01`.
    #[must_use]
    pub fn gate_eligible(&self) -> bool {
        self.lane == Lane::Public
    }

    /// Instance counts keyed by `(family, state code)`.
    #[must_use]
    pub fn state_counts(&self) -> BTreeMap<(IdentityFamily, &'static str), usize> {
        let mut m = BTreeMap::new();
        for i in &self.instances {
            *m.entry((i.family, i.state.code())).or_insert(0) += 1;
        }
        m
    }

    /// Instance count in one state code across all families.
    #[must_use]
    pub fn count_state(&self, code: &str) -> usize {
        self.instances
            .iter()
            .filter(|i| i.state.code() == code)
            .count()
    }

    #[must_use]
    pub fn alternate_counts(&self) -> AlternateCounts {
        let mut a = AlternateCounts {
            footing_consistent: 0,
            rate_consistent: 0,
        };
        for i in &self.instances {
            let footing = matches!(
                i.family,
                IdentityFamily::TccFooting | IdentityFamily::AiscFooting
            );
            match &i.state {
                IdentityState::Consistent {
                    alternate_consistent: true,
                    ..
                } if footing => a.footing_consistent += 1,
                IdentityState::IssuerDiscrepancy(c) if c.alternate_consistent => {
                    a.footing_consistent += 1
                }
                IdentityState::Consistent {
                    alternate_consistent: true,
                    ..
                } => a.rate_consistent += 1,
                _ => {}
            }
        }
        a
    }

    /// One (ii), (iii) or (iv) instance as a JSON object; `scope` labels a hidden-scope instance.
    fn exception_json(&self, i: &IdentityInstance, scope: Option<&str>) -> String {
        let mut o = format!(
            "{{\"state\":{},\"label\":{},\"file\":{},\"sha256\":{},\"sheet\":{},\"cell\":{},\
             \"subject\":{},\"family\":{},\"detail\":{}",
            js(i.state.code()),
            js(i.state.label()),
            js(&i.file),
            js(&self.source_sha(&i.file)),
            js(&i.sheet),
            js(&i.cell),
            js(&i.subject),
            js(i.family.as_str()),
            js(&i.state.detail())
        );
        if let Some(scope) = scope {
            let _ = write!(o, ",\"scope\":{}", js(scope));
        }
        match &i.state {
            IdentityState::IssuerDiscrepancy(c) => {
                let lines: Vec<String> = c.line_cells.iter().map(|l| js(l)).collect();
                let _ = write!(
                    o,
                    ",\"certificate\":{{\"printed\":{},\"component_sum\":{},\"residual\":{},\"total_cell\":{},\
                     \"line_cells\":[{}],\"sign_flip_witness\":{},\"alternate_consistent\":{}}}",
                    c.printed,
                    c.component_sum,
                    c.residual,
                    js(&c.total_cell),
                    lines.join(","),
                    c.sign_flip_witness
                        .as_deref()
                        .map_or_else(|| "null".to_string(), js),
                    c.alternate_consistent
                );
            }
            IdentityState::Explained { rule, .. } => {
                let _ = write!(o, ",\"rule\":{}", js(rule));
            }
            IdentityState::Undecidable {
                missing_fact,
                closing_document,
                ..
            } => {
                let _ = write!(
                    o,
                    ",\"missing_fact\":{},\"closing_document\":{}",
                    js(missing_fact),
                    closing_document
                        .as_deref()
                        .map_or_else(|| "null".to_string(), js)
                );
            }
            _ => {}
        }
        o.push('}');
        o
    }

    /// sha256 over the sorted `"{file}\t{sha256}\n"` lines of the sources this run read (BD-339.S1).
    #[must_use]
    pub fn registry_epoch(&self) -> String {
        let mut lines: Vec<String> = self
            .sources
            .iter()
            .map(|s| format!("{}\t{}\n", s.file_name, hex(&s.workbook.sha256)))
            .collect();
        lines.sort();
        sha256(lines.concat().as_bytes()).to_hex()
    }

    /// sha256 over `"{file}\t{numeric_digest}\n"` in registry order: one value an independent reader of the
    /// cells can reproduce (BD-339.S1).
    #[must_use]
    pub fn census_digest_of_digests(&self) -> String {
        let lines: String = self
            .sources
            .iter()
            .map(|s| format!("{}\t{}\n", s.file_name, hex(&s.census.numeric_digest)))
            .collect();
        sha256(lines.as_bytes()).to_hex()
    }

    /// Compares this run with a row of the expected-roots file: root, anchor, totals, registry epoch and digest
    /// of digests must all be equal. The error names every field that differs.
    pub fn check_expected(&self, expected: &ExpectedRoot) -> Result<(), String> {
        let totals = STATE_CODES.map(|c| self.count_state(c) as u64);
        let checks = [
            (
                "state_root",
                self.ledger_anchor.state_root.to_hex(),
                expected.state_root.clone(),
            ),
            (
                "anchor_id",
                self.ledger_anchor.anchor_id().to_string(),
                expected.anchor_id.clone(),
            ),
            (
                "totals",
                format!("{totals:?}"),
                format!("{:?}", expected.totals),
            ),
            (
                "registry_epoch",
                self.registry_epoch(),
                expected.registry_epoch.clone(),
            ),
            (
                "census_digest_of_digests",
                self.census_digest_of_digests(),
                expected.census_digest_of_digests.clone(),
            ),
        ];
        let differ: Vec<String> = checks
            .iter()
            .filter(|(_, got, want)| got != want)
            .map(|(name, got, want)| format!("{name}: run {got}, expected {want}"))
            .collect();
        if differ.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "run differs from the expected row of commit {} (line {}): {}",
                expected.commit,
                expected.line,
                differ.join("; ")
            ))
        }
    }

    fn source_sha(&self, file: &str) -> String {
        self.sources
            .iter()
            .find(|s| s.file_name == file)
            .map(|s| hex(&s.workbook.sha256))
            .unwrap_or_default()
    }

    /// Canonical JSON payload for the CLI envelope.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut o = String::new();
        // The no-claim line, the four flags and the next actions are envelope fields (fmine-cli), not payload.
        let _ = write!(
            o,
            "{{\"issuer\":{},\"lane\":{},\"gate_eligible\":{},",
            js(&self.issuer),
            js(self.lane.as_str()),
            self.gate_eligible()
        );
        let _ = write!(
            o,
            "\"retrieved_utc\":{},\"through\":{},\"sources\":[",
            js(&self.retrieved_utc),
            self.through
                .as_deref()
                .map_or_else(|| "null".to_string(), js)
        );
        for (n, s) in self.sources.iter().enumerate() {
            let c = &s.census;
            let _ = write!(
                o,
                "{}{{\"file\":{},\"kind\":{},\"quarter\":{},\"url\":{},\"url_basis\":{},\"bytes\":{},\
                 \"sha256\":{},\"visible_sheets\":{},\"hidden_sheets\":{},\"numeric_cells\":{},\
                 \"placeholder_cells\":{},\"formula_error_cells\":{},\"text_cells\":{},\"numeric_digest\":{}}}",
                if n == 0 { "" } else { "," },
                js(&s.file_name),
                js(s.kind.as_str()),
                js(&s.quarter),
                js(&s.url),
                js(&s.url_basis),
                s.bytes,
                js(&hex(&s.workbook.sha256)),
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
                js(&hex(&c.numeric_digest))
            );
        }
        let total =
            |f: fn(&CellCensus) -> usize| self.sources.iter().map(|s| f(&s.census)).sum::<usize>();
        let _ = write!(
            o,
            "],\"extraction\":{{\"workbooks\":{},\"visible_sheets\":{},\"numeric_cells\":{},\
             \"placeholder_cells\":{},\"formula_error_cells\":{},\"text_cells\":{}}},",
            self.sources.len(),
            total(|c| c.visible_sheets),
            total(|c| c.numeric_cells),
            total(|c| c.placeholder_cells),
            total(|c| c.formula_error_cells),
            total(|c| c.text_cells)
        );
        o.push_str("\"mine_stats\":[");
        for (n, (file, read)) in self.mine_stats.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"quarter\":{},\"blocks\":{},\"cells\":{},\"number\":{},\"numeric_zero\":{},\
                 \"nil\":{},\"na\":{},\"missing\":{},\"digest\":{},\"hidden_sheets\":[{}],\"refusals\":[",
                if n == 0 { "" } else { "," },
                js(file),
                js(&read.quarter),
                read.blocks,
                read.values.len(),
                read.count("number"),
                read.numeric_zeros(),
                read.count("nil"),
                read.count("na"),
                read.count("missing"),
                js(&read.digest().to_hex()),
                read.hidden_sheets
                    .iter()
                    .map(|h| js(h))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for (k, r) in read.refusals.iter().enumerate() {
                let _ = write!(
                    o,
                    "{}{{\"locator\":{},\"reason\":{}}}",
                    if k == 0 { "" } else { "," },
                    js(&r.locator),
                    js(&r.reason)
                );
            }
            o.push_str("]}");
        }
        o.push_str("],");

        let counts = self.state_counts();
        let alt = self.alternate_counts();
        let _ = write!(
            o,
            "\"identity_ledger\":{{\"construction\":\"component_sum\",\"aisc_tables\":{},\"instances\":{},\
             \"by_family\":{{",
            self.aisc_tables,
            self.instances.len()
        );
        for (n, family) in IdentityFamily::ALL.iter().enumerate() {
            let _ = write!(
                o,
                "{}{}:{{\"basis\":{}",
                if n == 0 { "" } else { "," },
                js(family.as_str()),
                js(family.basis())
            );
            for code in STATE_CODES {
                let _ = write!(
                    o,
                    ",{}:{}",
                    js(code),
                    counts.get(&(*family, code)).copied().unwrap_or(0)
                );
            }
            o.push('}');
        }
        o.push_str("},\"totals\":{");
        for (n, code) in STATE_CODES.iter().enumerate() {
            let _ = write!(
                o,
                "{}{}:{}",
                if n == 0 { "" } else { "," },
                js(code),
                self.count_state(code)
            );
        }
        let _ = write!(
            o,
            "}},\"alternate_construction\":{{\"scored\":false,\"footing_consistent\":{},\
             \"rate_consistent_of_declared_consistent\":{}}},\"exceptions\":[",
            alt.footing_consistent, alt.rate_consistent
        );
        let exceptions: Vec<String> = self
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .map(|i| self.exception_json(i, None))
            .collect();
        o.push_str(&exceptions.join(","));
        o.push_str("],");
        for (key, code) in [("refused", "v"), ("not_published", "np")] {
            let mut by_reason: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for i in self.instances.iter().filter(|i| i.state.code() == code) {
                by_reason.entry(i.state.detail()).or_default().push(format!(
                    "{}:{}!{}:{}",
                    i.file,
                    i.sheet,
                    i.cell,
                    i.family.as_str()
                ));
            }
            let _ = write!(o, "{}:[", js(key));
            for (n, (reason, cells)) in by_reason.iter().enumerate() {
                let cells: Vec<String> = cells.iter().map(|c| js(c)).collect();
                let _ = write!(
                    o,
                    "{}{{\"reason\":{},\"count\":{},\"instances\":[{}]}}",
                    if n == 0 { "" } else { "," },
                    js(reason),
                    cells.len(),
                    cells.join(",")
                );
            }
            o.push_str("],");
        }
        o.push_str("\"unregistered_headers\":[");
        for (n, h) in self.unregistered_headers.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"sheet\":{},\"cell\":{},\"header\":{}}}",
                if n == 0 { "" } else { "," },
                js(&h.file),
                js(&h.sheet),
                js(&h.cell),
                js(&h.header)
            );
        }
        o.push_str("],\"table_refusals\":[");
        for (n, t) in self.table_refusals.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"sheet\":{},\"reason\":{}}}",
                if n == 0 { "" } else { "," },
                js(&t.file),
                js(&t.sheet),
                js(&t.reason)
            );
        }
        o.push_str("]},");
        let hidden = &self.hidden_scope;
        let _ = write!(
            o,
            "\"hidden_scope\":{{\"scope\":{},\"counted\":false,\"aisc_tables\":{},\"instances\":{},\"by_state\":{{",
            js(HIDDEN_SCOPE),
            hidden.aisc_tables,
            hidden.instances.len()
        );
        for (n, code) in STATE_CODES.iter().enumerate() {
            let count = hidden
                .instances
                .iter()
                .filter(|i| i.state.code() == *code)
                .count();
            let _ = write!(o, "{}{}:{count}", if n == 0 { "" } else { "," }, js(code));
        }
        let hidden_exceptions: Vec<String> = hidden
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .map(|i| self.exception_json(i, Some(HIDDEN_SCOPE)))
            .collect();
        let _ = write!(
            o,
            "}},\"exceptions\":[{}],\"unregistered_headers\":{},\"table_refusals\":{}}},\"quarantine\":[",
            hidden_exceptions.join(","),
            hidden.unregistered_headers.len(),
            hidden.table_refusals.len()
        );
        let mut first = true;
        for s in &self.sources {
            for q in &s.workbook.quarantined {
                let item = match q {
                    QuarantineItem::HiddenSheet { sheet_name, state } => format!(
                        "{{\"file\":{},\"kind\":\"hidden_sheet\",\"sheet\":{},\"state\":{}}}",
                        js(&s.file_name),
                        js(sheet_name),
                        js(state)
                    ),
                    QuarantineItem::FormulaErrorCell {
                        sheet_name,
                        cell_ref,
                        error_token,
                    } => format!(
                        "{{\"file\":{},\"kind\":\"formula_error\",\"sheet\":{},\"cell\":{},\"token\":{}}}",
                        js(&s.file_name),
                        js(sheet_name),
                        js(cell_ref),
                        js(error_token)
                    ),
                };
                if !first {
                    o.push(',');
                }
                o.push_str(&item);
                first = false;
            }
        }
        let _ = write!(
            o,
            "],\"state_root\":{},\"anchor_id\":{}}}",
            js(&self.ledger_anchor.state_root.to_hex()),
            js(&self.ledger_anchor.anchor_id().to_string())
        );
        o
    }

    /// The commands worth running after this report, as `(command, why)`: the CLI puts them in the
    /// envelope's `next_actions`.
    #[must_use]
    pub fn next_actions(&self) -> Vec<(&'static str, &'static str)> {
        let other_lane = match self.lane {
            Lane::Public => (
                "fmine assure public --synthetic",
                "known-answer run on synthetic workbooks that exercises every ledger state",
            ),
            Lane::Synthetic => (
                "fmine assure public --issuer barrick",
                "the gate-eligible run over the workbooks in registries/sources/barrick.toml",
            ),
        };
        vec![
            other_lane,
            ("fmine robot-docs", "verbs, envelope fields and exit codes"),
        ]
    }
}

const STATE_CODES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];

/// What an `assure public` report does not claim; the CLI prints it in the envelope's `no_claim` field.
pub const NO_CLAIM: &str = "Reproduces and checks the issuer's published AISC reconciliation tables as printed. \
It makes no reserve, resource, geotechnical, regulatory or investment claim and implies no issuer affiliation.";

/// JSON string literal.
fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Tries a registered reconstruction for a per-ounce miss. `None` when none is registered for the instance;
/// otherwise (ii) when every label check passes and the component closes the interval, or (iv) naming the
/// check that failed.
fn try_reconstruction(
    sources: &[LoadedSource],
    recs: &[RegisteredReconstruction],
    file: &str,
    sheet: &str,
    subject: &str,
    family: IdentityFamily,
    miss: RateMiss,
) -> Option<IdentityState> {
    let rec = recs.iter().find(|r| {
        r.table_file == file && r.sheet == sheet && r.subject == subject && r.family == family
    })?;
    let fail = |why: String| {
        Some(IdentityState::Undecidable {
            detail: format!(
                "registered reconstruction {} not applied: {why}",
                rec.evidence_id
            ),
            missing_fact: DEFAULT_MISSING_FACT.to_string(),
            closing_document: None,
        })
    };
    if let ExplanationBasis::Stated {
        doc,
        locator,
        quote_prefix,
    } = &rec.basis
    {
        // A stated rule counts only while the issuer's text still says it (BD-331.S2).
        let quote = locator.split_once('!').and_then(|(sheet, cell)| {
            let ws = sources
                .iter()
                .find(|s| s.file_name == *doc)?
                .workbook
                .sheets
                .iter()
                .find(|s| s.name == sheet)?;
            parse_a1_ref(cell).and_then(|(r, c)| cell_text(ws, r, c))
        });
        if !quote
            .as_deref()
            .is_some_and(|t| t.starts_with(quote_prefix))
        {
            return fail(format!(
                "stated quote {doc}:{locator} reads {:?}, expected text starting {quote_prefix:?}",
                quote.unwrap_or_default()
            ));
        }
    }
    let Some(src) = sources.iter().find(|s| s.file_name == rec.component_file) else {
        return fail(format!("{} not loaded", rec.component_file));
    };
    let Some(cs) = src
        .workbook
        .sheets
        .iter()
        .find(|s| s.name == rec.component_sheet)
    else {
        return fail(format!("no visible sheet '{}'", rec.component_sheet));
    };
    for (cell, prefix) in &rec.label_checks {
        let text = parse_a1_ref(cell).and_then(|(r, c)| cell_text(cs, r, c));
        if !text.as_deref().is_some_and(|t| t.starts_with(prefix)) {
            return fail(format!(
                "{}!{cell} reads {:?}, expected text starting {prefix:?}",
                rec.component_sheet,
                text.unwrap_or_default()
            ));
        }
    }
    let value = parse_a1_ref(&rec.component_cell)
        .and_then(|(r, c)| cs.cell(r, c))
        .and_then(|c| c.value.as_number());
    let c = match value {
        Some(v) if v.fract() == 0.0 && v.abs() < 1e15 => v as i64,
        _ => {
            return fail(format!(
                "{}!{} is not a whole number",
                rec.component_sheet, rec.component_cell
            ));
        }
    };
    let component = format!(
        "{}:{}!{}={c}",
        rec.component_file, rec.component_sheet, rec.component_cell
    );
    if reconstruction_closes(miss.n, miss.k, miss.q, miss.r, c) {
        Some(IdentityState::Explained {
            evidence_id: rec.evidence_id.clone(),
            rule: rec.rule.clone(),
            component,
            basis: rec.basis.clone(),
        })
    } else {
        fail(format!(
            "adding {component} does not close the rate interval"
        ))
    }
}

/// What the run knows about each subject's ownership share for the release `quarter` (BD-331.S3): regions as
/// such, and each registered share confirmed or not against the same-release Mine Stats quarter-to-date label.
fn share_bases(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    quarter: &str,
) -> BTreeMap<String, ShareBasis> {
    let mut out: BTreeMap<String, ShareBasis> = profile
        .regions
        .iter()
        .map(|r| (r.clone(), ShareBasis::Region))
        .collect();
    if profile.shares.is_empty() {
        return out;
    }
    let stats = sources
        .iter()
        .find(|s| s.kind == SourceKind::MineStats && s.quarter == quarter);
    let qtd = match stats {
        None => Err(format!("no Mine Stats workbook loaded for {quarter}")),
        Some(s) => find_qtd_sheet(&s.workbook.sheets, quarter)
            .map(|sheet| (s.file_name.as_str(), sheet))
            .map_err(|why| format!("{}: {why}", s.file_name)),
    };
    for share in &profile.shares {
        let basis = match &qtd {
            Err(why) => ShareBasis::Unconfirmed { why: why.clone() },
            Ok((file, sheet)) => {
                match confirm_share_label(sheet, &share.label, share.num, share.den) {
                    Ok(cell) => ShareBasis::Confirmed {
                        num: share.num,
                        den: share.den,
                        evidence: share.evidence.clone(),
                        label_cell: format!("{file}:{}!{cell}", sheet.name),
                    },
                    Err(why) => ShareBasis::Unconfirmed {
                        why: format!("{file}: {why}"),
                    },
                }
            }
        };
        out.insert(share.subject.clone(), basis);
    }
    out
}

/// Builds the identity ledger over every AISC table in `sources` and commits the run to a state root.
#[must_use]
pub fn assess(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
) -> PublicAssuranceReport {
    let issuer = profile.issuer.as_str();
    let subjects = profile.subjects.as_slice();
    let reconstructions = profile.reconstructions.as_slice();
    let mut visible = HiddenScope::default();
    let mut hidden = HiddenScope::default();
    for src in sources
        .iter()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        let shares = share_bases(profile, &sources, &src.quarter);
        let scopes = [
            (&src.workbook.sheets, &mut visible),
            (&src.workbook.hidden_sheets, &mut hidden),
        ];
        for (sheets, scope) in scopes {
            for sheet in sheets {
                match locate_table(sheet, subjects) {
                    Ok(None) => {}
                    Err(reason) => scope.table_refusals.push(TableRefusal {
                        file: src.file_name.clone(),
                        sheet: sheet.name.clone(),
                        reason,
                    }),
                    Ok(Some(table)) => {
                        scope.aisc_tables += 1;
                        let mut explain =
                            |subject: &str, family: IdentityFamily, miss: RateMiss| {
                                try_reconstruction(
                                    &sources,
                                    reconstructions,
                                    &src.file_name,
                                    &sheet.name,
                                    subject,
                                    family,
                                    miss,
                                )
                            };
                        ledger_for_table(
                            &src.file_name,
                            sheet,
                            &table,
                            subjects,
                            &shares,
                            &mut explain,
                            &mut scope.instances,
                            &mut scope.unregistered_headers,
                        );
                    }
                }
            }
        }
    }
    for i in visible.instances.iter_mut().chain(&mut hidden.instances) {
        name_missing_fact(i, &profile.missing_facts);
    }
    let HiddenScope {
        aisc_tables,
        instances,
        unregistered_headers: unregistered,
        table_refusals,
    } = visible;

    // The state root binds every input digest, every census and every instance's state, so changing any
    // input cell changes the root. Version 3 adds the hidden-scope section (BD-331.S2).
    let mut enc = Encoder::new_frame(*b"M0AS", 3);
    enc.put_str(issuer);
    enc.put_str(lane.as_str());
    enc.put_u64(sources.len() as u64);
    for s in &sources {
        enc.put_str(&s.file_name);
        enc.put_fixed(&s.workbook.sha256);
        enc.put_fixed(&s.census.numeric_digest);
        for n in [
            s.census.visible_sheets,
            s.census.hidden_sheets,
            s.census.numeric_cells,
            s.census.placeholder_cells,
            s.census.formula_error_cells,
            s.census.text_cells,
        ] {
            enc.put_u64(n as u64);
        }
    }
    for (instances, unregistered, table_refusals) in [
        (&instances, &unregistered, &table_refusals),
        (
            &hidden.instances,
            &hidden.unregistered_headers,
            &hidden.table_refusals,
        ),
    ] {
        enc.put_u64(instances.len() as u64);
        for i in instances {
            for field in [
                i.file.as_str(),
                &i.sheet,
                &i.cell,
                &i.subject,
                i.family.as_str(),
                i.state.code(),
            ] {
                enc.put_str(field);
            }
            enc.put_str(&i.state.detail());
        }
        enc.put_u64(unregistered.len() as u64);
        for h in unregistered {
            enc.put_str(&format!("{}:{}!{}={}", h.file, h.sheet, h.cell, h.header));
        }
        enc.put_u64(table_refusals.len() as u64);
        for t in table_refusals {
            enc.put_str(&format!("{}:{}:{}", t.file, t.sheet, t.reason));
        }
    }
    let state_root = blake3_hash(&enc.finish());
    let ledger_anchor = ledger_anchor(issuer, lane, sources.len(), state_root);
    let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
    let vocabulary = (!vocabulary.is_empty()).then_some(vocabulary);
    let mine_stats = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .map(|s| {
            // The registry already checks the quarter format; a refusal here is kept, never dropped.
            let read = read_mine_stats(&s.workbook, &s.quarter, vocabulary.as_ref())
                .unwrap_or_else(|refusal| MineStatsRead {
                    quarter: s.quarter.clone(),
                    blocks: 0,
                    values: Vec::new(),
                    refusals: vec![CellRefusal {
                        locator: s.file_name.clone(),
                        reason: refusal.to_string(),
                    }],
                    hidden_sheets: Vec::new(),
                });
            (s.file_name.clone(), read)
        })
        .collect();
    PublicAssuranceReport {
        issuer: issuer.to_string(),
        lane,
        retrieved_utc: profile.retrieved_utc.clone(),
        through: profile.through.clone(),
        sources,
        aisc_tables,
        table_refusals,
        instances,
        unregistered_headers: unregistered,
        mine_stats,
        hidden_scope: hidden,
        ledger_anchor,
    }
}

/// The anchor a run commits to: everything but the state root is fixed by the issuer, the lane and the number of
/// sources read.
fn ledger_anchor(issuer: &str, lane: Lane, sources: usize, state_root: Digest32) -> LedgerAnchor {
    let cell_id = match lane {
        Lane::Public => format!("CELL-PUBLIC({issuer})"),
        Lane::Synthetic => "CELL-SYNTHETIC".to_string(),
    };
    LedgerAnchor {
        format_version: FMT_ANCHOR_V1,
        cell_id,
        ledger_epoch: 1,
        commit_sequence: sources as u64,
        adapter_registry_epoch: 1,
        schema_epoch: 1,
        policy_epoch: 1,
        privacy_epoch: 1,
        residency_epoch: 1,
        state_root,
    }
}

/// Names the fact that would decide a (iv) instance: the registered `(subject, family)` row if there is one,
/// otherwise the text the ledger gave it (its family default, or the stated share rule's).
fn name_missing_fact(instance: &mut IdentityInstance, registered: &[RegisteredMissingFact]) {
    let IdentityState::Undecidable {
        missing_fact,
        closing_document,
        ..
    } = &mut instance.state
    else {
        return;
    };
    if let Some(row) = registered
        .iter()
        .find(|m| m.subject == instance.subject && m.family == instance.family)
    {
        *missing_fact = format!("{} ({})", row.text, row.evidence);
        closing_document.clone_from(&row.closing_document);
    }
}

/// `fmine assure public --issuer <issuer>`: the gate-eligible run over the registered public workbooks.
///
/// The issuer name `synthetic` is reserved for the invented workbooks: a registry under that name (for
/// example one written by [`synthetic::write_issuer_workspace`]) runs in the synthetic lane and is never
/// gate-eligible, though it goes through the same registry and digest checks.
pub fn run_public_assurance(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_registered_sources(profile, root)?;
    let lane = if profile.issuer == synthetic::SYNTHETIC_ISSUER {
        Lane::Synthetic
    } else {
        Lane::Public
    };
    Ok(assess(profile, lane, sources))
}

/// The Barrick M0 run (`GATE-BULL-01`): registry and corpus both under `root`.
pub fn run_barrick_m0_assurance(root: &Path) -> Result<PublicAssuranceReport, Refusal> {
    run_public_assurance(&sources::load_issuer_profile(root, "barrick")?, root)
}

/// `fmine assure public --synthetic`: the known-answer run over invented workbooks, with optional edits to
/// the synthetic AISC table (perturbation tests).
pub fn run_synthetic_assurance(
    edits: &[(String, fmine_io::xlsx::MinimalCell)],
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_synthetic(synthetic::synthetic_workbooks(edits))?;
    Ok(assess(
        &synthetic::synthetic_profile(),
        Lane::Synthetic,
        sources,
    ))
}

/// Parses in-memory synthetic workbooks `(file, kind, quarter, bytes)` into loaded sources.
fn load_synthetic(
    files: Vec<(&'static str, SourceKind, &'static str, Vec<u8>)>,
) -> Result<Vec<LoadedSource>, Refusal> {
    let mut sources = Vec::new();
    for (file, kind, quarter, bytes) in files {
        let workbook = Workbook::from_bytes(file, &bytes)
            .map_err(|e| Refusal::from_code("FMINE-REF-502", format!("synthetic {file}: {e}")))?;
        let census = workbook.cell_census();
        sources.push(LoadedSource {
            file_name: file.to_string(),
            kind,
            quarter: quarter.to_string(),
            url: String::new(),
            url_basis: "synthetic".to_string(),
            bytes: bytes.len() as u64,
            workbook,
            census,
        });
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_io::xlsx::MinimalCell;
    use std::path::PathBuf;

    fn state_of<'a>(
        r: &'a PublicAssuranceReport,
        cell: &str,
        family: IdentityFamily,
    ) -> &'a IdentityState {
        let found = r
            .instances
            .iter()
            .find(|i| i.cell == cell && i.family == family)
            .map(|i| &i.state);
        match found {
            Some(s) => s,
            None => panic!("no instance at {cell} {family:?}"),
        }
    }

    #[test]
    fn synthetic_lane_reproduces_every_seeded_answer() {
        let r = run_synthetic_assurance(&[]).expect("synthetic run");
        assert_eq!(r.lane, Lane::Synthetic);
        assert!(!r.gate_eligible());
        assert_eq!(r.ledger_anchor.cell_id, "CELL-SYNTHETIC");
        assert_eq!((r.aisc_tables, r.instances.len()), (1, 50));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        // Columns K and L (BD-331.S3) add nine (i) and one (iv) to the 35 instances of columns B–H; column M
        // (BD-331.S2) adds four (i) and one stated (ii).
        assert_eq!(totals, vec![34, 2, 1, 2, 6, 5]);

        match state_of(&r, "C8", IdentityFamily::TccFooting) {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum, c.residual), (350, 250, 100));
                assert_eq!(c.sign_flip_witness.as_deref(), Some("C6"));
                assert_eq!(c.line_cells, vec!["C5", "C6", "C7"]);
                assert!(!c.alternate_consistent);
            }
            other => panic!("C8 TCC footing: {other:?}"),
        }
        match state_of(&r, "D16", IdentityFamily::AiscPerOunce) {
            IdentityState::Undecidable {
                detail,
                missing_fact,
                closing_document,
            } => {
                // 405000/199 = 2035.1758…: the upper bound rounds up (it printed 2035.175 before 2026-10-29).
                assert!(detail.contains("[1965.174, 2035.176]"), "{detail}");
                // Site C's registered missing fact replaces the family default (BD-331.S2).
                assert_eq!(
                    missing_fact,
                    "the sustaining capital the synthetic issuer used for this rate (synthetic)"
                );
                assert_eq!(closing_document, &None);
            }
            other => panic!("D16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "E14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: "no registered share"
            }
        );
        assert_eq!(state_of(&r, "E15", IdentityFamily::TccPerOunce).code(), "i");
        // Stated share 3/4: K prints 1000·¾·400/150 = 2000 (i); L prints 2200 (iv).
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce),
            &IdentityState::Consistent {
                alternate_consistent: true,
                basis: Some(
                    "stated share 3/4 (synthetic); footnote Q1 2030!A30; label \
                     Synthetic_Mine_Stats.xlsx:QTD!A5"
                        .to_string()
                ),
            }
        );
        match state_of(&r, "L14", IdentityFamily::CosPerOunce) {
            s @ IdentityState::Undecidable {
                detail,
                missing_fact,
                ..
            } => {
                assert!(detail.contains("allow s in [0.8210, 0.8290]"), "{detail}");
                assert_eq!(missing_fact, aisc::SHARE_MISSING_FACT);
                assert!(
                    s.detail().ends_with(
                        "; missing fact: the attributable cost of sales or ounces the issuer used"
                    ),
                    "{}",
                    s.detail()
                );
            }
            other => panic!("L14: {other:?}"),
        }
        for (cell, family) in [
            ("G8", IdentityFamily::TccFooting),
            ("G11", IdentityFamily::AiscFooting),
            ("G14", IdentityFamily::CosPerOunce),
            ("G15", IdentityFamily::TccPerOunce),
            ("G16", IdentityFamily::AiscPerOunce),
        ] {
            assert_eq!(
                state_of(&r, cell, family),
                &IdentityState::Refused {
                    reason: "unrounded stored value; needs cell number format"
                }
            );
        }
        assert!(
            r.instances
                .iter()
                .filter(|i| i.column == "F")
                .all(|i| i.state.code() == "np")
        );
        match state_of(&r, "H16", IdentityFamily::AiscPerOunce) {
            IdentityState::Explained { component, .. } => {
                assert_eq!(component, "Synthetic_Mine_Stats.xlsx:Minestats3!B32=13");
            }
            other => panic!("H16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "M16", IdentityFamily::AiscPerOunce).label(),
            "explained_stated"
        );
        assert_eq!(r.unregistered_headers.len(), 1);
        assert_eq!(r.unregistered_headers[0].cell, "J2");
        assert!(r.instances.iter().all(|i| i.column != "I"));
        let aisc = r
            .sources
            .iter()
            .find(|s| s.file_name == synthetic::SYNTHETIC_AISC_FILE)
            .expect("aisc");
        assert!(
            aisc.workbook
                .quarantined
                .iter()
                .any(|q| matches!(q, QuarantineItem::HiddenSheet { .. }))
        );
        assert!(
            aisc.workbook
                .quarantined
                .iter()
                .any(|q| matches!(q, QuarantineItem::FormulaErrorCell { .. }))
        );

        let json = r.to_json();
        assert!(json.starts_with(
            "{\"issuer\":\"synthetic\",\"lane\":\"synthetic\",\"gate_eligible\":false"
        ));
        assert!(json.contains("\"sign_flip_witness\":\"C6\""));
        assert!(
            // Site J (BD-331.S2) adds four (i) and one stated (ii); the hidden table is not counted.
            json.contains("\"totals\":{\"i\":34,\"ii\":2,\"iii\":1,\"iv\":2,\"v\":6,\"np\":5}")
        );
        assert!(json.contains(
            "\"hidden_scope\":{\"scope\":\"sheet not displayed to readers\",\"counted\":false,\
             \"aisc_tables\":1,\"instances\":5,\"by_state\":{\"i\":4,\"ii\":0,\"iii\":1,\"iv\":0,\"v\":0,\"np\":0}"
        ));
        // The flags, the no-claim line and the next actions belong to the CLI envelope, not the payload.
        for field in ["\"flags\"", "\"no_claim\"", "\"next_actions\""] {
            assert!(!json.contains(field), "{field} in the payload");
        }
        assert!(json.ends_with(&format!(
            "\"anchor_id\":\"{}\"}}",
            r.ledger_anchor.anchor_id()
        )));
    }

    /// A cell changed in an input moves the state root and the affected instance's state.
    #[test]
    fn perturbing_an_input_cell_moves_the_root_and_the_state() {
        let base = run_synthetic_assurance(&[]).expect("base");
        assert_eq!(
            state_of(&base, "B8", IdentityFamily::TccFooting).code(),
            "i"
        );
        let edited = run_synthetic_assurance(&[("B8".to_string(), MinimalCell::Number(401.0))])
            .expect("edited");
        assert_ne!(
            base.ledger_anchor.state_root,
            edited.ledger_anchor.state_root
        );
        match state_of(&edited, "B8", IdentityFamily::TccFooting) {
            IdentityState::IssuerDiscrepancy(c) => assert_eq!(c.residual, 1),
            other => panic!("B8 after edit: {other:?}"),
        }
        // A change outside every table still moves the root (it is in the numeric digest).
        let far =
            run_synthetic_assurance(&[("Z99".to_string(), MinimalCell::Number(1.0))]).expect("far");
        assert_ne!(base.ledger_anchor.state_root, far.ledger_anchor.state_root);
        // And the run is deterministic.
        let again = run_synthetic_assurance(&[]).expect("again");
        assert_eq!(
            base.ledger_anchor.state_root,
            again.ledger_anchor.state_root
        );
    }

    /// A component whose labels do not match keeps the instance undecidable and says why.
    #[test]
    fn reconstruction_with_a_wrong_label_does_not_apply() {
        let sources = run_synthetic_assurance(&[]).expect("base").sources;
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions[0].label_checks = vec![
            ("A32".to_string(), "Minesite sustaining".to_string()),
            ("B3".to_string(), "2031".to_string()),
        ];
        let r = assess(&profile, Lane::Synthetic, sources);
        match state_of(&r, "H16", IdentityFamily::AiscPerOunce) {
            IdentityState::Undecidable { detail, .. } => assert!(detail.contains("B3"), "{detail}"),
            other => panic!("H16: {other:?}"),
        }
    }

    /// The registry is read from the workspace; the corpus is absent. Every registered file is named, and the
    /// count comes from the registry, not from a literal.
    #[test]
    fn missing_corpus_refuses_naming_every_registered_file() {
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let n = profile.sources.len();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("no-such-workspace");
        let refusal = run_public_assurance(&profile, &root).expect_err("must refuse");
        assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-502", 8));
        assert!(
            refusal
                .what
                .contains(&format!("{n} of {n} registered barrick workbooks unusable")),
            "{}",
            refusal.what
        );
        for src in &profile.sources {
            assert!(
                refusal.what.contains(&src.file_name),
                "{} not named",
                src.file_name
            );
        }
    }

    /// A row added to a copy of the registry is picked up with no code change: the refusal count follows the
    /// registry, and the added file is named.
    #[test]
    fn test_refusal_count_follows_registry_rows() {
        let ws = std::env::temp_dir().join(format!("fmine-public-rows-{}", std::process::id()));
        let reg_dir = ws.join("registries").join("sources");
        std::fs::create_dir_all(&reg_dir).expect("temp registry dir");
        let text = std::fs::read_to_string(sources::registry_path(&workspace_root(), "barrick"))
            .expect("read");
        let base = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .sources
            .len();
        let added = "\n[[source]]\nfile = \"Barrick_Q3_2026_Mine_Stats.xlsx\"\nkind = \"mine_stats\"\n\
                     quarter = \"2026Q3\"\npath = \"2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx\"\n\
                     url_basis = \"W1 naming rule\"\nbytes = 1\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000001\"\n";
        std::fs::write(reg_dir.join("barrick.toml"), format!("{text}{added}")).expect("write");
        let profile = load_issuer_profile(&ws, "barrick").expect("extended registry");
        assert_eq!(profile.sources.len(), base + 1);
        let refusal = run_public_assurance(&profile, &ws).expect_err("no corpus");
        let n = base + 1;
        assert!(
            refusal
                .what
                .contains(&format!("{n} of {n} registered barrick workbooks unusable"))
        );
        assert!(refusal.what.contains("Barrick_Q3_2026_Mine_Stats.xlsx"));
        // `--through 2026Q2` drops the added row again.
        assert_eq!(
            profile.through("2026Q2").expect("bound").sources.len(),
            base
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// The synthetic workbooks with Site H's `QTD` label replaced, assessed under `profile`.
    fn synthetic_run_with(
        profile: &IssuerProfile,
        edits: &[(String, MinimalCell)],
        site_h_label: &str,
    ) -> PublicAssuranceReport {
        let mut files = synthetic::synthetic_workbooks(edits);
        for f in &mut files {
            if f.0 == synthetic::SYNTHETIC_STATS_FILE {
                f.3 = synthetic::synthetic_stats_workbook(site_h_label);
            }
        }
        let sources = load_synthetic(files).expect("synthetic workbooks parse");
        assess(profile, Lane::Synthetic, sources)
    }

    const NO_FOOTNOTE: IdentityState = IdentityState::Refused {
        reason: "attributable-basis footnote not found on the sheet",
    };

    /// BD-331.S3: a registered share the same-release label does not print is (v), naming the cell.
    #[test]
    fn test_share_label_mismatch_gives_v() {
        let profile = synthetic::synthetic_profile();
        let r = synthetic_run_with(&profile, &[], "Site H (80%)1");
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce),
            &IdentityState::RefusedAt {
                reason: "registered share not confirmed by label",
                at: "Synthetic_Mine_Stats.xlsx: QTD!A5 prints 80%, registered 3/4".to_string(),
            }
        );
        // Site I's label still prints 75%, so its rate is still checked.
        assert_eq!(
            state_of(&r, "L14", IdentityFamily::CosPerOunce).code(),
            "iv"
        );
        let r = synthetic_run_with(&profile, &[], "Site Hx (75%)");
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce).detail(),
            "registered share not confirmed by label: Synthetic_Mine_Stats.xlsx: no column-A label \
             'Site H (x%)' on QTD"
        );
    }

    /// BD-331.S3: the footnote is found by what it says, whatever its letter or row.
    #[test]
    fn test_footnote_located_by_text_not_letter() {
        let other_note = (
            String::from("A30"),
            MinimalCell::Text("h.  Ounces sold are in thousands.".to_string()),
        );
        let moved = [
            other_note.clone(),
            (
                String::from("A41"),
                MinimalCell::Text(synthetic::SYNTHETIC_SHARE_FOOTNOTE.replacen("h.", "k.", 1)),
            ),
        ];
        let r = run_synthetic_assurance(&moved).expect("synthetic");
        match state_of(&r, "K14", IdentityFamily::CosPerOunce) {
            IdentityState::Consistent { basis: Some(b), .. } => {
                assert!(b.contains("footnote Q1 2030!A41"), "{b}")
            }
            other => panic!("K14: {other:?}"),
        }
        // Planted negative: letter "h" stays at A30, but nothing states the attributable basis.
        let r = run_synthetic_assurance(&[other_note]).expect("synthetic");
        for cell in ["K14", "L14"] {
            assert_eq!(
                state_of(&r, cell, IdentityFamily::CosPerOunce),
                &NO_FOOTNOTE
            );
        }
    }

    /// BD-331.S3: a region's cost-of-sales rate stays (v), now for the reason that applies.
    #[test]
    fn test_regions_keep_v_new_reason() {
        let nci = [(String::from("H7"), MinimalCell::Number(-10.0))];
        let r = run_synthetic_assurance(&nci).expect("synthetic");
        assert_eq!(
            state_of(&r, "H14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: "regional numerator needs member weighting"
            }
        );
        // Unregistered as a region, the same column is refused for having no share instead.
        let mut profile = synthetic::synthetic_profile();
        profile.regions.clear();
        let r = synthetic_run_with(&profile, &nci, "Site H (75%)1");
        assert_eq!(
            state_of(&r, "H14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: "no registered share"
            }
        );
        assert!(
            r.instances
                .iter()
                .all(|i| i.state.detail() != "attributable share needed")
        );
    }

    /// BD-331.S3 mutation: without its registered share, Site H's rate is (v), not consistent.
    #[test]
    fn test_drop_registered_share_yields_v() {
        let mut profile = synthetic::synthetic_profile();
        profile.shares.retain(|s| s.subject != "Site H");
        let r = synthetic_run_with(&profile, &[], "Site H (75%)1");
        assert_eq!(
            state_of(&r, "K14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: "no registered share"
            }
        );
        assert_eq!(
            state_of(&r, "L14", IdentityFamily::CosPerOunce).code(),
            "iv"
        );
    }

    /// BD-331.S2: a (ii) says whether the issuer states its rule. Region North's is reconstructed; Site J's is
    /// stated at A31. Barrick's E816 rows are registered as reconstructed.
    #[test]
    fn test_explained_basis_labels() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let h16 = state_of(&r, "H16", IdentityFamily::AiscPerOunce);
        assert_eq!(h16.label(), "explained_reconstructed");
        assert_eq!(
            h16.detail(),
            "SYNTHETIC: Synthetic_Mine_Stats.xlsx:Minestats3!B32=13"
        );
        let m16 = state_of(&r, "M16", IdentityFamily::AiscPerOunce);
        assert_eq!(m16.label(), "explained_stated");
        assert_eq!(
            m16.detail(),
            "SYNTHETIC-STATED: Synthetic_Mine_Stats.xlsx:Minestats3!B32=13; stated at \
             Synthetic_AISC.xlsx:Q1 2030!A31"
        );
        let json = r.to_json();
        assert!(json.contains("\"label\":\"explained_stated\""), "{json}");
        let barrick = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let e816: Vec<&ExplanationBasis> = barrick
            .reconstructions
            .iter()
            .filter(|r| r.evidence_id == "E816")
            .map(|r| &r.basis)
            .collect();
        assert_eq!(e816, [&ExplanationBasis::Reconstructed; 3]);
    }

    /// BD-331.S2 planted negative: when the stated quote no longer reads as registered, the explanation is
    /// refused and the instance stays (iv), naming the quote cell. The reconstructed (ii) needs no quote.
    #[test]
    fn test_stated_quote_mismatch_keeps_iv() {
        for text in [
            "i.  Site J's all-in sustaining costs per ounce exclude Site G's capital.",
            "",
        ] {
            let edit = [(String::from("A31"), MinimalCell::Text(text.to_string()))];
            let r = run_synthetic_assurance(&edit).expect("synthetic");
            match state_of(&r, "M16", IdentityFamily::AiscPerOunce) {
                IdentityState::Undecidable {
                    detail,
                    missing_fact,
                    ..
                } => {
                    assert!(
                        detail.starts_with(
                            "registered reconstruction SYNTHETIC-STATED not applied: stated quote \
                             Synthetic_AISC.xlsx:Q1 2030!A31 reads"
                        ),
                        "{detail}"
                    );
                    assert_eq!(missing_fact, aisc::DEFAULT_MISSING_FACT);
                }
                other => panic!("M16 with A31 = {text:?}: {other:?}"),
            }
            assert_eq!(
                state_of(&r, "H16", IdentityFamily::AiscPerOunce).label(),
                "explained_reconstructed"
            );
        }
    }

    /// BD-331.S2: an unregistered (subject, family) names its family's default; a registered row replaces it;
    /// a stated-share miss names the share rule's fact.
    #[test]
    fn test_missing_fact_family_default() {
        let mut profile = synthetic::synthetic_profile();
        profile.missing_facts.clear();
        let r = synthetic_run_with(&profile, &[], "Site H (75%)1");
        let fact = |cell: &str, family| match state_of(&r, cell, family) {
            IdentityState::Undecidable { missing_fact, .. } => missing_fact.clone(),
            other => panic!("{cell}: {other:?}"),
        };
        assert_eq!(
            fact("D16", IdentityFamily::AiscPerOunce),
            aisc::DEFAULT_MISSING_FACT
        );
        assert_eq!(
            fact("L14", IdentityFamily::CosPerOunce),
            aisc::SHARE_MISSING_FACT
        );
        // A row for another family of the same subject does not apply.
        profile.missing_facts = synthetic::synthetic_profile().missing_facts;
        profile.missing_facts[0].family = IdentityFamily::TccPerOunce;
        let r = synthetic_run_with(&profile, &[], "Site H (75%)1");
        match state_of(&r, "D16", IdentityFamily::AiscPerOunce) {
            IdentityState::Undecidable { missing_fact, .. } => {
                assert_eq!(missing_fact, aisc::DEFAULT_MISSING_FACT)
            }
            other => panic!("D16: {other:?}"),
        }
        // Every (iv) of the seeded run names one, in the state and in the report.
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let iv: Vec<&IdentityInstance> = r
            .instances
            .iter()
            .filter(|i| i.state.code() == "iv")
            .collect();
        assert_eq!(iv.len(), 2);
        for i in iv {
            assert!(i.state.detail().contains("; missing fact: the "), "{i:?}");
        }
        assert_eq!(r.to_json().matches("\"missing_fact\":\"the ").count(), 2);
    }

    /// BD-331.S2: missing-fact texts are prose. The two built-in texts and every registered Barrick row have no
    /// digit (the registry refuses one; `fmine-registry` tests that).
    #[test]
    fn test_missing_fact_texts_have_no_digits() {
        let barrick = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        assert_eq!(barrick.missing_facts.len(), 3);
        let texts = [aisc::DEFAULT_MISSING_FACT, aisc::SHARE_MISSING_FACT]
            .into_iter()
            .chain(barrick.missing_facts.iter().map(|m| m.text.as_str()));
        for text in texts {
            assert!(!text.bytes().any(|b| b.is_ascii_digit()), "{text}");
        }
    }

    /// BD-331.S2: hidden sheets are parsed into `hidden_sheets` and leave the census and its digest unchanged.
    #[test]
    fn test_hidden_sheet_parsed_not_censused() {
        let files = synthetic::synthetic_workbooks(&[]);
        let aisc_bytes = files
            .iter()
            .find(|f| f.0 == synthetic::SYNTHETIC_AISC_FILE)
            .map(|f| f.3.clone())
            .expect("aisc");
        let with_hidden = Workbook::from_bytes("a.xlsx", &aisc_bytes).expect("parse");
        let visible_only = Workbook::from_bytes(
            "a.xlsx",
            &fmine_io::xlsx::build_minimal_xlsx(&[fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells: synthetic::synthetic_aisc_cells(),
            }]),
        )
        .expect("parse");
        let names: Vec<&str> = with_hidden
            .hidden_sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["Scratch", synthetic::SYNTHETIC_HIDDEN_SHEET]);
        assert_eq!(
            with_hidden.hidden_sheets[0]
                .cell(1, 1)
                .and_then(|c| c.value.as_number()),
            Some(1.0)
        );
        assert!(with_hidden.sheet("Scratch").is_none());
        let (a, b) = (with_hidden.cell_census(), visible_only.cell_census());
        assert_eq!((a.hidden_sheets, b.hidden_sheets), (2, 0));
        assert_eq!(
            CellCensus {
                hidden_sheets: 0,
                ..a
            },
            b
        );
    }

    /// BD-331.S2: the hidden table's instances are reported in their own section, labelled, and never counted.
    #[test]
    fn test_hidden_scope_never_counted() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let hidden = &r.hidden_scope;
        assert_eq!(hidden.aisc_tables, 1);
        assert_eq!(r.aisc_tables, 1);
        let states: Vec<(&str, &str)> = hidden
            .instances
            .iter()
            .map(|i| (i.cell.as_str(), i.state.code()))
            .collect();
        assert_eq!(
            states,
            [
                ("B8", "iii"),
                ("B11", "i"),
                ("B14", "i"),
                ("B15", "i"),
                ("B16", "i")
            ]
        );
        assert!(
            hidden
                .instances
                .iter()
                .all(|i| i.sheet == synthetic::SYNTHETIC_HIDDEN_SHEET)
        );
        assert!(
            r.instances
                .iter()
                .all(|i| i.sheet == synthetic::SYNTHETIC_TABLE_SHEET)
        );
        assert_eq!(r.count_state("iii"), 1, "only C8 on the visible sheet");
        let json = r.to_json();
        let (ledger, rest) = json.split_once("\"hidden_scope\":").expect("section");
        assert!(!ledger.contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
        assert!(rest.contains(
            "\"sheet\":\"Q4 2029\",\"cell\":\"B8\",\"subject\":\"Site A\",\"family\":\"tcc_footing\",\
             \"detail\":\"printed 401 vs component sum 400 (residual 1)\",\
             \"scope\":\"sheet not displayed to readers\""
        ));
    }

    /// BD-331.S2: a hidden cell is outside the counts but inside the root.
    #[test]
    fn test_hidden_scope_moves_root() {
        let base = run_synthetic_assurance(&[]).expect("synthetic");
        let mut files = synthetic::synthetic_workbooks(&[]);
        let mut hidden_cells = synthetic::synthetic_hidden_cells();
        hidden_cells.retain(|(r, _)| r != "B8");
        hidden_cells.push(("B8".to_string(), MinimalCell::Number(400.0)));
        for f in &mut files {
            if f.0 == synthetic::SYNTHETIC_AISC_FILE {
                f.3 = fmine_io::xlsx::build_minimal_xlsx(&[
                    fmine_io::xlsx::MinimalSheet {
                        name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                        hidden: false,
                        cells: synthetic::synthetic_aisc_cells(),
                    },
                    fmine_io::xlsx::MinimalSheet {
                        name: "Scratch".to_string(),
                        hidden: true,
                        cells: vec![("A1".to_string(), MinimalCell::Number(1.0))],
                    },
                    fmine_io::xlsx::MinimalSheet {
                        name: synthetic::SYNTHETIC_HIDDEN_SHEET.to_string(),
                        hidden: true,
                        cells: hidden_cells.clone(),
                    },
                ]);
            }
        }
        let moved = assess(
            &synthetic::synthetic_profile(),
            Lane::Synthetic,
            load_synthetic(files).expect("parse"),
        );
        assert_eq!(moved.instances, base.instances);
        assert_eq!(
            moved.sources[1].census.numeric_digest,
            base.sources[1].census.numeric_digest
        );
        assert_eq!(moved.hidden_scope.instances[0].state.code(), "i");
        assert_ne!(
            moved.ledger_anchor.state_root,
            base.ledger_anchor.state_root
        );
    }

    /// A complete run through a TOML registry in default CI (the real-corpus runs are ignored tests): the two
    /// synthetic workbooks plus a third, later Mine Stats file are written to a scratch corpus, registered by
    /// digest in a generated TOML, and loaded with no code-side profile. The ledger equals the in-code synthetic
    /// profile's, and `--through 2030Q1` drops the third file and leaves the ledger unchanged.
    #[test]
    fn test_synthetic_three_file_toml_profile_runs_end_to_end() {
        let ws = std::env::temp_dir().join(format!("fmine-public-toml-e2e-{}", std::process::id()));
        let reg = synthetic::write_issuer_workspace(&ws).expect("write workspace");
        assert_eq!(reg, sources::registry_path(&ws, "synthetic"));
        let code_profile = synthetic::synthetic_profile();

        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        assert_eq!(profile.sources.len(), 3);
        assert_eq!(profile.reconstructions, code_profile.reconstructions);
        assert_eq!(profile.missing_facts, code_profile.missing_facts);
        let r = run_public_assurance(&profile, &ws).expect("all three files verify by digest");
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        assert_eq!(
            totals,
            vec![34, 2, 1, 2, 6, 5],
            "same ledger as the in-code synthetic profile"
        );
        assert_eq!(r.hidden_scope.instances.len(), 5);
        assert_eq!(
            (&profile.regions, &profile.shares),
            (&code_profile.regions, &code_profile.shares)
        );
        assert_eq!(r.sources.len(), 3);
        assert_eq!(r.ledger_anchor.commit_sequence, 3);
        // The reserved issuer name keeps invented numbers out of the public lane; the same registry under
        // another name would be gate-eligible, so the name is what decides.
        assert_eq!((r.lane, r.gate_eligible()), (Lane::Synthetic, false));
        let mut renamed = profile.clone();
        renamed.issuer = "renamed".to_string();
        let public = run_public_assurance(&renamed, &ws).expect("same files");
        assert_eq!((public.lane, public.gate_eligible()), (Lane::Public, true));

        let bounded =
            run_public_assurance(&profile.through("2030Q1").expect("bound"), &ws).expect("run");
        assert_eq!(bounded.sources.len(), 2);
        assert_eq!(bounded.through.as_deref(), Some("2030Q1"));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| bounded.count_state(c)).collect();
        assert_eq!(totals, vec![34, 2, 1, 2, 6, 5]);
        assert_ne!(
            bounded.ledger_anchor.state_root, r.ledger_anchor.state_root,
            "the source set is in the root"
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// bd-jg43.1 parity: reading styles changes no census count and no numeric digest. The values were printed
    /// by this test at commit 345e2ba, before the styles reader existed, and pasted here.
    #[test]
    fn test_census_unchanged_by_style_reading() {
        let got: Vec<String> = synthetic::synthetic_workbooks(&[])
            .into_iter()
            .map(|(name, _, _, bytes)| {
                let c = Workbook::from_bytes(name, &bytes)
                    .expect("synthetic workbook")
                    .cell_census();
                format!(
                    "{name} {} {} {} {} {} {} {}",
                    c.visible_sheets,
                    c.hidden_sheets,
                    c.numeric_cells,
                    c.placeholder_cells,
                    c.formula_error_cells,
                    c.text_cells,
                    hex(&c.numeric_digest)
                )
            })
            .collect();
        let want = [
            "Synthetic_Mine_Stats.xlsx 2 0 2 0 0 9 ed492958121280b991212c61d01c9c2a257a551dfb07af145f1f6c117e646c21",
            "Synthetic_AISC.xlsx 1 2 107 4 1 28 7db9df6e5bceec22d9195ea3680e67a774b97275dcc4be0f4e67580828ee79ed",
        ];
        assert_eq!(got, want, "{got:#?}");
    }

    fn kit_readme() -> String {
        std::fs::read_to_string(
            workspace_root()
                .join("repro")
                .join("barrick")
                .join("README.md"),
        )
        .expect("kit README")
    }

    /// BD-339.S4 golden: the kit README's generated section equals the generator's output from the code's row
    /// anchors and the registry. On a mismatch it prints the section to paste; tests never write files.
    #[test]
    fn test_kit_readme_selectors_match_code() {
        let readme = kit_readme();
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let want = sources::kit_selectors_markdown(&profile);
        let got = sources::kit_readme_section(&readme).expect("both markers");
        assert!(
            got == want.trim_matches('\n'),
            "repro/barrick/README.md's generated section differs; paste this between the markers:\n{}\n{}{}",
            sources::KIT_SELECTORS_BEGIN,
            want,
            sources::KIT_SELECTORS_END
        );
    }

    /// BD-339.S4: the golden comparison sees a one-subject drift in either direction.
    #[test]
    fn test_kit_readme_golden_detects_drift() {
        let readme = kit_readme();
        let section = sources::kit_readme_section(&readme).expect("both markers");
        let mut profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let first = profile.subjects[0].clone();
        assert_eq!(
            section,
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
        let renamed = readme.replacen(&format!("\n- {first}\n"), "\n- Renamed subject\n", 1);
        assert_ne!(renamed, readme, "the subject line is in the README");
        assert_ne!(
            sources::kit_readme_section(&renamed).expect("both markers"),
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
        profile.subjects.remove(0);
        assert_ne!(
            section,
            sources::kit_selectors_markdown(&profile).trim_matches('\n')
        );
    }

    /// BD-339.S4: the kit README carries the no-affiliation sentence verbatim and the RISK-31 instruction.
    #[test]
    fn test_kit_readme_states_no_affiliation_and_risk_31() {
        let readme = kit_readme();
        assert!(readme.contains(
            "FrankenMining is not affiliated with Barrick; these are Barrick's public files, fetched from \
             Barrick's host"
        ));
        assert!(
            readme.contains("RISK-31: do not redistribute the files; fetch them from the issuer.")
        );
    }

    /// BD-339.S1: the expected-roots file parses and every row is well formed. Each anchor is the one the run
    /// derives from its root (18 sources, public lane), so a typo in either is caught without the corpus.
    #[test]
    fn test_expected_roots_file_is_well_formed() {
        let rows = sources::load_expected_roots(&sources::expected_roots_path(
            &workspace_root(),
            "barrick",
        ))
        .expect("expected roots");
        assert!(rows.len() >= 5, "rows are append-only");
        let hex64 = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let registry = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        for row in &rows {
            let at = format!("row of {} (line {})", row.commit, row.line);
            assert!(
                row.commit.len() >= 7 && row.commit.bytes().all(|b| b.is_ascii_hexdigit()),
                "{at}"
            );
            for h in [
                &row.registry_epoch,
                &row.state_root,
                &row.census_digest_of_digests,
            ] {
                assert!(hex64(h), "{at}: {h}");
            }
            assert!(!row.reason.is_empty() && !row.evidence.is_empty(), "{at}");
            assert_eq!(row.totals.iter().sum::<u64>(), 2595, "{at}");
            let mut root = [0_u8; 32];
            for (i, byte) in root.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&row.state_root[2 * i..2 * i + 2], 16).expect("hex");
            }
            let root = Digest32(root);
            assert_eq!(
                ledger_anchor("barrick", Lane::Public, registry.sources.len(), root)
                    .anchor_id()
                    .to_string(),
                row.anchor_id,
                "{at}"
            );
        }
        // E822's row, resolved by re-running commit 748da79 on the corpus.
        assert_eq!(
            (rows[0].commit.as_str(), rows[0].totals),
            ("748da79", [2120, 3, 4, 17, 391, 60])
        );
        let roots: std::collections::BTreeSet<&str> =
            rows.iter().map(|r| r.state_root.as_str()).collect();
        assert_eq!(roots.len(), rows.len(), "each row records a new root");
    }

    /// BD-339.S1 negative control: one flipped hex digit in any compared field is reported, naming the field.
    #[test]
    fn test_expected_root_comparison_rejects_flipped_digit() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let good = ExpectedRoot {
            commit: "0000000".to_string(),
            registry_epoch: r.registry_epoch(),
            state_root: r.ledger_anchor.state_root.to_hex(),
            anchor_id: r.ledger_anchor.anchor_id().to_string(),
            totals: STATE_CODES.map(|c| r.count_state(c) as u64),
            census_digest_of_digests: r.census_digest_of_digests(),
            reason: "synthetic".to_string(),
            evidence: "synthetic".to_string(),
            line: 1,
        };
        assert_eq!(r.check_expected(&good), Ok(()));
        let flip = |s: &str| {
            let mut b = s.as_bytes().to_vec();
            let last = b.len() - 1;
            b[last] = if b[last] == b'0' { b'1' } else { b'0' };
            String::from_utf8(b).expect("ascii")
        };
        let cases: [(&str, ExpectedRoot); 5] = [
            (
                "state_root",
                ExpectedRoot {
                    state_root: flip(&good.state_root),
                    ..good.clone()
                },
            ),
            (
                "anchor_id",
                ExpectedRoot {
                    anchor_id: flip(&good.anchor_id),
                    ..good.clone()
                },
            ),
            (
                "registry_epoch",
                ExpectedRoot {
                    registry_epoch: flip(&good.registry_epoch),
                    ..good.clone()
                },
            ),
            (
                "census_digest_of_digests",
                ExpectedRoot {
                    census_digest_of_digests: flip(&good.census_digest_of_digests),
                    ..good.clone()
                },
            ),
            (
                "totals",
                ExpectedRoot {
                    totals: [good.totals[0] + 1, 0, 0, 0, 0, 0],
                    ..good.clone()
                },
            ),
        ];
        for (field, bad) in cases {
            let why = r.check_expected(&bad).expect_err(field);
            assert!(why.contains(&format!("{field}: run ")), "{field}: {why}");
        }
    }

    #[test]
    fn test_next_actions_text_has_no_hard_coded_count() {
        let actions = run_synthetic_assurance(&[])
            .expect("synthetic")
            .next_actions();
        let text: String = actions.iter().map(|(c, w)| format!("{c} {w} ")).collect();
        assert!(text.contains("registries/sources/barrick.toml"), "{text}");
        assert!(!text.contains(" 18 "), "{text}");
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// GATE-BULL-01 on the real registered workbooks. Every expected value below comes from the independent
    /// Python reader written before this code (scratch `fidelity.py` and `census2.py`, 2026-10-29; recorded in
    /// MOR_CASE E820–E823), never from this implementation's own output.
    ///
    /// The run is bounded `--through 2026Q2`, so it keeps testing the same inputs after later vintages are
    /// registered. The state root at the end is different in kind: it is a regression pin of this
    /// implementation's own output (MOR_CASE E822, updated 2026-10-29 for outward rate rounding), not an
    /// independent value.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_real_files_match_the_independent_reader() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        assert!(r.gate_eligible());
        assert_eq!(r.through.as_deref(), Some("2026Q2"));

        // Part 1, extraction fidelity: visible, hidden, numeric, placeholder, error, text cells and the digest.
        let oracle: &[(&str, [usize; 6], &str)] = &[
            (
                "Barrick_Q1_2024_Mine_Stats.xlsx",
                [6, 0, 1130, 298, 0, 251],
                "293c9e29be4e09cc0eb380b2d80bcbca5aae5656bd65284f275b3265dafc2691",
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                [6, 0, 1141, 197, 0, 243],
                "f4636a96aa9f97c2e29969c5cd551b107e6a887129454074a527d27315b788a7",
            ),
            (
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                [6, 0, 1112, 242, 0, 249],
                "e96c6202baddf1fea717d8b8ae3fb55211f5a1fe4e35f152f4e8d92eca0dc7a3",
            ),
            (
                "Barrick_Q2_2024_Mine_Stats.xlsx",
                [7, 0, 2299, 556, 0, 490],
                "23e8da7796431d1a13cbbace051ffd7fb048f3dc1e96311b55e2c27853cf0b19",
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                [7, 0, 2304, 370, 0, 475],
                "e251c3f9b10fd4355e91d4518d02c94d27da097d80e6e3045f99bcb6a66845d0",
            ),
            (
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                [7, 0, 2297, 480, 1092, 494],
                "cab9164142f442cd240606ae7a0652c8953f449c9a4b98f2f8cf081e255ad3b2",
            ),
            (
                "Barrick_Q3_2024_Mine_Stats.xlsx",
                [7, 0, 2305, 550, 0, 489],
                "7b035d292541cd6693239816ea07ed9fb68930887052d8fcd5ffb3baedfc085c",
            ),
            (
                "Barrick_Q3_2025_Mine_Stats.xlsx",
                [7, 0, 2311, 363, 0, 475],
                "e18d37f3b0df75d238a227519e40ffac5e553b00684e489b135e834b11856ad3",
            ),
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                [7, 0, 2308, 546, 0, 489],
                "bbf38e19e04a4d3d1bfe9fe0b26afba69863065873e6459d716e67328038a3b1",
            ),
            (
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                [7, 0, 2357, 350, 0, 486],
                "ddb6af2ae1508e85d01ed3445fb3ce54883e3791a5510427b8c21cdcadef9efb",
            ),
            (
                "Barrick_Q1_2025_AISC.xlsx",
                [3, 0, 1146, 0, 0, 243],
                "fb649ed9d8e09f54f4569c482266e1f33c2017f2045d5d2f65671b5ba0112cfe",
            ),
            (
                "Barrick_Q1_2026_AISC.xlsx",
                [3, 2, 680, 0, 0, 154],
                "38dcba35d809105e525637133ca349722502e36d4facce667c2810d54d63e574",
            ),
            (
                "Barrick_Q2_2025_AISC.xlsx",
                [5, 0, 1910, 0, 0, 400],
                "fdabe9e9ca38b3caa89b14b769ea1f74777ca59220af04b2355d9ac7f6a66fc9",
            ),
            (
                "Barrick_Q2_2026_AISC.xlsx",
                [5, 2, 1088, 0, 0, 254],
                "dbef0e1bab051189f7fed9a7cd6e90ff8aff51ee3ad6a8d3f3181a6e53e59ffb",
            ),
            (
                "Barrick_Q3_2024_AISC.xlsx",
                [5, 0, 1910, 0, 0, 410],
                "9a7c8fe4b3298c63a4abfd7dc89ab87933df588aca80941f7930713b945c1076",
            ),
            (
                "Barrick_Q3_2025_AISC.xlsx",
                [5, 0, 1610, 0, 0, 395],
                "82d1d5c8aae8e9718da38ae52704a49a16dea07e227d70050bb862127c7c9849",
            ),
            (
                "Barrick_Q4_2024_AISC.xlsx",
                [4, 1, 1528, 0, 0, 328],
                "8431c88ef3a474f1c97c4a3485669292145e28f5a37536b56c0e7d79b9f9df0d",
            ),
            (
                "Barrick_Q4_2025_AISC.xlsx",
                [4, 1, 952, 0, 0, 212],
                "954f03ad421d6d20ca4c062e5b11b239ee3c46e97e5ceb4cb49dade616f71d1e",
            ),
        ];
        assert_eq!(
            r.sources.len(),
            oracle.len(),
            "every loaded file has an oracle row"
        );
        for &(file, counts, digest) in oracle {
            let s = r.sources.iter().find(|s| s.file_name == file).expect(file);
            let c = &s.census;
            let got = [
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
            ];
            assert_eq!(got, counts, "{file} census");
            assert_eq!(hex(&c.numeric_digest), digest, "{file} numeric digest");
        }

        // Part 2, the identity ledger.
        assert_eq!((r.aisc_tables, r.instances.len()), (34, 2595));
        assert!(r.table_refusals.is_empty() && r.unregistered_headers.is_empty());
        let counts = r.state_counts();
        let get = |f: IdentityFamily, c: &'static str| counts.get(&(f, c)).copied().unwrap_or(0);
        use IdentityFamily::{AiscFooting, AiscPerOunce, CosPerOunce, TccFooting, TccPerOunce};
        assert_eq!(
            [
                get(TccFooting, "i"),
                get(TccFooting, "iii"),
                get(TccFooting, "v"),
                get(TccFooting, "np")
            ],
            [502, 2, 3, 12]
        );
        assert_eq!(
            [
                get(AiscFooting, "i"),
                get(AiscFooting, "iii"),
                get(AiscFooting, "v"),
                get(AiscFooting, "np")
            ],
            [502, 2, 3, 12]
        );
        assert_eq!(
            [
                get(CosPerOunce, "i"),
                get(CosPerOunce, "iv"),
                get(CosPerOunce, "v"),
                get(CosPerOunce, "np")
            ],
            // BD-331.S3 (census E826): of the 379 rates refused before for a non-zero non-controlling-interest
            // line, 263 fit the stated share (i) and 65 miss (iv); the 51 region instances stay (v).
            [111 + 263, 17 + 65, 379 - 328, 12]
        );
        assert_eq!(
            [
                get(TccPerOunce, "i"),
                get(TccPerOunce, "v"),
                get(TccPerOunce, "np")
            ],
            [504, 3, 12]
        );
        assert_eq!(
            [
                get(AiscPerOunce, "i"),
                get(AiscPerOunce, "ii"),
                get(AiscPerOunce, "v"),
                get(AiscPerOunce, "np")
            ],
            [501, 3, 3, 12]
        );
        assert_eq!(
            r.alternate_counts(),
            AlternateCounts {
                footing_consistent: 1008,
                // A stated-share fit sums one line (k = 1), so the alternative construction accepts all 263.
                rate_consistent: 994 + 263
            }
        );

        let mut iii: Vec<(String, String, String, i64, i64)> = r
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                IdentityState::IssuerDiscrepancy(c) => Some((
                    i.file.clone(),
                    i.sheet.clone(),
                    i.cell.clone(),
                    c.printed,
                    c.component_sum,
                )),
                _ => None,
            })
            .collect();
        iii.sort();
        let col_of = |cell: &str| {
            cell.trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        };
        let found: Vec<(String, String, String, i64, i64)> = iii
            .into_iter()
            .map(|(f, s, c, p, sum)| (f, s, col_of(&c), p, sum))
            .collect();
        let want = |f: &str, c: &str, p: i64, s: i64| {
            (f.to_string(), "Q2 2024".to_string(), c.to_string(), p, s)
        };
        assert_eq!(
            found,
            vec![
                want("Barrick_Q2_2025_AISC.xlsx", "L", 13, 12),
                want("Barrick_Q2_2025_AISC.xlsx", "O", 70, 72),
                want("Barrick_Q3_2024_AISC.xlsx", "L", 13, 12),
                want("Barrick_Q3_2024_AISC.xlsx", "O", 70, 72),
            ]
        );
        let ii: Vec<String> = r
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                IdentityState::Explained { component, .. } => Some(format!(
                    "{}|{}|{}|{}",
                    i.file,
                    i.sheet,
                    component,
                    i.state.label()
                )),
                _ => None,
            })
            .collect();
        // BD-331.S2: no issuer note states E816's rule, so all three are reconstructed.
        assert_eq!(
            ii,
            vec![
                "Barrick_Q1_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10|\
                 explained_reconstructed",
                "Barrick_Q2_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10|\
                 explained_reconstructed",
                "Barrick_Q2_2025_AISC.xlsx|YTD Q2 2025|Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B74=13|\
                 explained_reconstructed",
            ]
        );
        let iv_subjects: Vec<&str> = r
            .instances
            .iter()
            .filter(|i| i.state.code() == "iv")
            .map(|i| i.subject.as_str())
            .collect();
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Veladero").count(), 12);
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Hemlo").count(), 3);
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Kibali").count(), 2);

        // Regression pin (not independent; see the doc comment): root, anchor, totals, registry epoch and digest
        // of digests equal the last row of repro/barrick/expected.toml, which records why each root moved.
        let expected = sources::load_expected_roots(&sources::expected_roots_path(
            &workspace_root(),
            "barrick",
        ))
        .expect("expected roots");
        let last = expected.last().expect("at least one row");
        if let Err(why) = r.check_expected(last) {
            panic!("{why}");
        }
    }

    /// bd-jg43.1 on the real files: `Workbook::format_census` equals the independent reader
    /// (`python3 -I scripts/oracle/formats.py`, committed at 345e2ba before the Rust existed), run here on the
    /// same files. Lines are compared whole in the oracle's canonical JSON. The totals are also checked against
    /// the table written into the bead before either reader ran. One line per file, one per mismatch, then a
    /// summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_format_census_matches_independent_reader() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let mut names: Vec<&str> = profile
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names.len(), 18);
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|n| root.join(&profile.corpus_dir).join(n))
            .collect();
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/formats.py"))
            .args(&paths)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "formats.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let oracle: Vec<&str> = stdout.lines().collect();
        assert_eq!(oracle.len(), 19, "18 file lines and a summary");

        let render = |formats: &BTreeMap<String, fmine_io::xlsx::FormatCount>| {
            let body: Vec<String> = formats
                .iter()
                .map(|(code, c)| format!("{}:[{},{}]", js(code), c.numeric, c.non_integer))
                .collect();
            format!("{{{}}}", body.join(","))
        };
        let mut total: BTreeMap<String, fmine_io::xlsx::FormatCount> = BTreeMap::new();
        let mut mismatches = 0;
        for ((name, path), want) in names.iter().zip(&paths).zip(&oracle) {
            let wb = Workbook::from_file(path).expect("registered workbook");
            let census = wb.format_census();
            for (code, c) in &census {
                let t = total.entry(code.clone()).or_default();
                t.numeric += c.numeric;
                t.non_integer += c.non_integer;
            }
            let got = format!(
                "{{\"file\":{},\"sha256\":\"{}\",\"formats\":{}}}",
                js(name),
                hex(&wb.sha256),
                render(&census)
            );
            let equal = got == *want;
            let open = got.strip_suffix('}').expect("rendered object");
            println!("{open},\"oracle_equal\":{equal}}}");
            if !equal {
                mismatches += 1;
                println!("{{\"file\":{},\"rust\":{got},\"oracle\":{want}}}", js(name));
            }
        }
        let numeric_total: usize = total.values().map(|c| c.numeric).sum();
        let non_integer_total: usize = total.values().map(|c| c.non_integer).sum();
        let summary = format!(
            "{{\"files\":18,\"formats\":{},\"numeric_total\":{numeric_total},\"non_integer_total\":{non_integer_total}}}",
            render(&total)
        );
        let pass = mismatches == 0 && summary == oracle[18];
        let open = summary.strip_suffix('}').expect("rendered object");
        println!("{open},\"pass\":{pass}}}");
        assert_eq!(
            mismatches, 0,
            "per-file format census differs from formats.py"
        );
        assert_eq!(summary, oracle[18]);
        // The bead's table (bd-jg43.1, written from a scratch read before either reader): 10 formats, 30,388
        // numeric cells (E820's total), 6,256 of them non-integer.
        assert_eq!(
            (total.len(), numeric_total, non_integer_total),
            (10, 30_388, 6_256)
        );
    }

    /// BD-331.S2 on the real files: every (iv), visible or hidden, names a missing fact. One JSON line per (iv)
    /// (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_every_iv_names_missing_fact() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        let mut by_fact: BTreeMap<(String, String), usize> = BTreeMap::new();
        for i in r.instances.iter().chain(&r.hidden_scope.instances) {
            let IdentityState::Undecidable { missing_fact, .. } = &i.state else {
                continue;
            };
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"subject\":{},\"family\":{},\"missing_fact\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                js(&i.subject),
                js(i.family.as_str()),
                js(missing_fact)
            );
            assert!(!missing_fact.is_empty());
            *by_fact
                .entry((i.subject.clone(), missing_fact.clone()))
                .or_default() += 1;
        }
        let visible_iv = r.count_state("iv");
        assert_eq!(visible_iv, 82);
        let velad = (
            "Veladero".to_string(),
            "the ounce series behind the per-ounce rows (E691)".to_string(),
        );
        let attributable = "the attributable ounces sold used for the rate (E817)".to_string();
        let visible = |subject: &str, fact: &str| {
            r.instances
                .iter()
                .filter(|i| {
                    i.subject == subject
                        && matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                            if missing_fact == fact)
                })
                .count()
        };
        assert_eq!(visible(&velad.0, &velad.1), 12);
        assert_eq!(visible("Kibali", &attributable), 2);
        assert_eq!(visible("Hemlo", &attributable), 3);
        let share = r
            .instances
            .iter()
            .filter(|i| {
                matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                    if missing_fact == aisc::SHARE_MISSING_FACT)
            })
            .count();
        assert_eq!(share, 65);
        println!(
            "{{\"visible_iv\":{visible_iv},\"hidden_iv\":{},\"by_subject_and_fact\":{}}}",
            r.hidden_scope
                .instances
                .iter()
                .filter(|i| i.state.code() == "iv")
                .count(),
            by_fact.len()
        );
    }

    /// BD-331.S2 on the real files: the hidden scope. Every count is from the hidden census
    /// (`python3 -I scripts/oracle/hidden_census.py .`, which runs census2.py and share_census.py unchanged on
    /// the hidden sheets). One JSON line per hidden instance and per hidden sheet, then a summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_hidden_scope_matches_census() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        let hidden = &r.hidden_scope;
        let mut sheets: BTreeMap<(String, String), BTreeMap<&str, usize>> = BTreeMap::new();
        let (mut footing, mut per_oz) = (BTreeMap::new(), BTreeMap::new());
        for i in &hidden.instances {
            println!(
                "{{\"file\":{},\"sheet\":{},\"column\":{},\"subject\":{},\"family\":{},\"state\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.column),
                js(&i.subject),
                js(i.family.as_str()),
                js(i.state.code())
            );
            *sheets
                .entry((i.file.clone(), i.sheet.clone()))
                .or_default()
                .entry(i.state.code())
                .or_default() += 1;
            let tally = match i.family {
                IdentityFamily::TccFooting | IdentityFamily::AiscFooting => &mut footing,
                _ => &mut per_oz,
            };
            *tally.entry(i.state.code()).or_insert(0_usize) += 1;
        }
        for ((file, sheet), by_state) in &sheets {
            let states: Vec<String> = by_state
                .iter()
                .map(|(k, v)| format!("{}:{v}", js(k)))
                .collect();
            println!(
                "{{\"file\":{},\"sheet\":{},\"instances\":{},\"by_state\":{{{}}}}}",
                js(file),
                js(sheet),
                by_state.values().sum::<usize>(),
                states.join(",")
            );
        }
        // The census's numbers (2026-10-30): 6 hidden sheets, 435 instances.
        let census_footing = BTreeMap::from([("i", 169), ("iii", 1), ("np", 4)]);
        let census_per_oz = BTreeMap::from([("i", 235), ("iv", 17), ("np", 6), ("v", 3)]);
        let agrees = sheets.len() == 6
            && hidden.instances.len() == 435
            && footing == census_footing
            && per_oz == census_per_oz;
        println!(
            "{{\"hidden_sheets\":{},\"instances\":{},\"iii\":{},\"census_agrees\":{agrees}}}",
            sheets.len(),
            hidden.instances.len(),
            footing.get("iii").copied().unwrap_or(0)
        );
        assert_eq!((hidden.aisc_tables, sheets.len()), (6, 6));
        assert_eq!(hidden.instances.len(), 435);
        assert_eq!(footing, census_footing);
        assert_eq!(per_oz, census_per_oz);
        assert!(hidden.unregistered_headers.is_empty() && hidden.table_refusals.is_empty());
        // E815, E821: North America Q4 2023 total cash costs, hidden in the Q4 2024 file, prints 541 against 540.
        let na = hidden
            .instances
            .iter()
            .find(|i| i.state.code() == "iii")
            .expect("one (iii)");
        assert_eq!(
            (na.file.as_str(), na.sheet.as_str(), na.column.as_str()),
            ("Barrick_Q4_2024_AISC.xlsx", "Q4 2023", "J")
        );
        assert_eq!(
            (na.subject.as_str(), na.family),
            ("North America", IdentityFamily::TccFooting)
        );
        match &na.state {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum), (541, 540))
            }
            other => panic!("{other:?}"),
        }
        // Never counted: the visible ledger is unchanged by the hidden scope.
        assert_eq!((r.aisc_tables, r.instances.len()), (34, 2595));
    }

    /// BD-331.S3 on the real files. Every count is from the independent census E826
    /// (`python3 -I scripts/oracle/share_census.py .`, written without reading this crate). Prints one JSON line
    /// per cost-of-sales rate the rule decides, then a summary line (`--nocapture` to see them).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_share_rule_matches_census() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        // fit, miss, unconfirmed, regions_v, no_registered_share, no_footnote
        let mut summary = [0_usize; 6];
        let mut by_subject: BTreeMap<String, [usize; 3]> = BTreeMap::new();
        for i in r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce)
        {
            let (slot, detail) = match &i.state {
                IdentityState::Consistent { basis: Some(b), .. } => (0, b.as_str()),
                IdentityState::Undecidable { detail, .. } if detail.contains("1000·s·N/Q") => {
                    (1, detail.as_str())
                }
                IdentityState::RefusedAt { at, .. } => (2, at.as_str()),
                IdentityState::Refused {
                    reason: "regional numerator needs member weighting",
                } => (3, ""),
                IdentityState::Refused {
                    reason: "no registered share",
                } => (4, ""),
                s if *s == NO_FOOTNOTE => (5, ""),
                IdentityState::Refused {
                    reason: "attributable share needed",
                } => panic!("{} {} {} keeps the old reason", i.file, i.sheet, i.cell),
                _ => continue,
            };
            summary[slot] += 1;
            if slot < 3 {
                by_subject.entry(i.subject.clone()).or_default()[slot] += 1;
            }
            let interval = detail
                .split_once("allow s in [")
                .and_then(|(_, rest)| rest.split_once(']'))
                .and_then(|(inside, _)| inside.split_once(", "));
            let (s_lo, s_hi) = interval.unwrap_or(("null", "null"));
            let share = detail
                .split_once("stated share ")
                .and_then(|(_, rest)| rest.split_once(' '))
                .map_or_else(|| "null".to_string(), |(s, _)| js(s));
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"subject\":{},\"share\":{share},\"state\":{},\
                 \"s_lo\":{s_lo},\"s_hi\":{s_hi},\"detail\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                js(&i.subject),
                js(i.state.code()),
                js(&i.state.detail())
            );
        }
        let [fit, miss, unconfirmed, regions_v, no_share, no_footnote] = summary;
        let sites = fit + miss + unconfirmed + no_share + no_footnote;
        println!(
            "{{\"sites\":{sites},\"fit\":{fit},\"miss\":{miss},\"unconfirmed\":{unconfirmed},\
             \"regions_v\":{regions_v},\"no_registered_share\":{no_share},\"no_footnote\":{no_footnote}}}"
        );
        assert_eq!((sites, summary), (328, [263, 65, 0, 51, 0, 0]));
        let census: BTreeMap<String, [usize; 3]> = [
            ("Bulyanhulu", 31, 3),
            ("Carlin", 9, 25),
            ("Cortez", 31, 3),
            ("Long Canyon", 3, 0),
            ("Loulo-Gounkoto", 23, 0),
            ("Nevada Gold Mines", 11, 23),
            ("North Mara", 30, 4),
            ("Phoenix", 34, 0),
            ("Pueblo Viejo", 29, 5),
            ("Tongon", 30, 0),
            ("Turquoise Ridge", 32, 2),
        ]
        .into_iter()
        .map(|(s, fit, miss)| (s.to_string(), [fit, miss, 0]))
        .collect();
        assert_eq!(by_subject, census);
    }

    #[test]
    fn public_claim_from_a_synthetic_cell() {
        let r = run_synthetic_assurance(&[]).expect("synthetic");
        let aisc = r
            .sources
            .iter()
            .find(|s| s.file_name == synthetic::SYNTHETIC_AISC_FILE)
            .expect("aisc");
        let sheet = aisc
            .workbook
            .sheet(synthetic::SYNTHETIC_TABLE_SHEET)
            .expect("sheet");
        let cell = sheet.cell(5, 2).expect("B5");
        let claim = cell_to_public_claim(
            &aisc.workbook,
            &sheet.name,
            "cos_usd_m",
            cell,
            AdditivityClass::ExtensiveAdditive,
        )
        .expect("claim");
        assert_eq!(claim.value, Some(500.0));
        assert_eq!(claim.anchors[0].locator, "Q1 2030!B5");
        let dash = sheet.cell(6, 6).expect("F6");
        let placeholder = cell_to_public_claim(
            &aisc.workbook,
            &sheet.name,
            "dep",
            dash,
            AdditivityClass::ExtensiveAdditive,
        )
        .expect("placeholder");
        assert_eq!(placeholder.value, None);
        let err = sheet.cell(20, 2).expect("B20");
        assert!(
            cell_to_public_claim(
                &aisc.workbook,
                &sheet.name,
                "x",
                err,
                AdditivityClass::ExtensiveAdditive
            )
            .is_err()
        );
    }
}
