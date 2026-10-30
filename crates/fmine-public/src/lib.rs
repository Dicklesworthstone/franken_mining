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
    IdentityFamily, IdentityInstance, IdentityState, RateMiss, UnregisteredHeader, cell_text,
    ledger_for_table, locate_table, reconstruction_closes,
};
use crate::mine_stats::{CellRefusal, MineStatsRead, read_mine_stats};
pub use crate::sources::load_issuer_profile;
use crate::sources::{IssuerProfile, RegisteredReconstruction, SourceKind};

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
    pub ledger_anchor: LedgerAnchor,
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
                } if footing => a.footing_consistent += 1,
                IdentityState::IssuerDiscrepancy(c) if c.alternate_consistent => {
                    a.footing_consistent += 1
                }
                IdentityState::Consistent {
                    alternate_consistent: true,
                } => a.rate_consistent += 1,
                _ => {}
            }
        }
        a
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
        let _ = write!(
            o,
            "{{\"issuer\":{},\"lane\":{},\"gate_eligible\":{},\"no_claim\":{},",
            js(&self.issuer),
            js(self.lane.as_str()),
            self.gate_eligible(),
            js(NO_CLAIM)
        );
        o.push_str(
            "\"flags\":{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
             \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false},",
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
        let mut first = true;
        for i in self
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
        {
            let _ = write!(
                o,
                "{}{{\"state\":{},\"label\":{},\"file\":{},\"sha256\":{},\"sheet\":{},\"cell\":{},\
                 \"subject\":{},\"family\":{},\"detail\":{}",
                if first { "" } else { "," },
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
                _ => {}
            }
            o.push('}');
            first = false;
        }
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
        o.push_str("]},\"quarantine\":[");
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
            "],\"state_root\":{},\"anchor_id\":{},\"next_actions\":[",
            js(&self.ledger_anchor.state_root.to_hex()),
            js(&self.ledger_anchor.anchor_id().to_string())
        );
        let actions: &[(&str, &str)] = match self.lane {
            Lane::Public => &[
                (
                    "fmine assure public --synthetic",
                    "known-answer run on synthetic workbooks that exercises every ledger state",
                ),
                ("fmine robot-docs", "verbs, envelope fields and exit codes"),
            ],
            Lane::Synthetic => &[
                (
                    "fmine assure public --issuer barrick",
                    "the gate-eligible run over the workbooks in registries/sources/barrick.toml",
                ),
                ("fmine robot-docs", "verbs, envelope fields and exit codes"),
            ],
        };
        for (n, (command, why)) in actions.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"command\":{},\"why\":{}}}",
                if n == 0 { "" } else { "," },
                js(command),
                js(why)
            );
        }
        o.push_str("]}");
        o
    }
}

const STATE_CODES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];

const NO_CLAIM: &str = "Reproduces and checks the issuer's published AISC reconciliation tables as printed. \
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
        })
    };
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
        })
    } else {
        fail(format!(
            "adding {component} does not close the rate interval"
        ))
    }
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
    let mut instances = Vec::new();
    let mut unregistered = Vec::new();
    let mut table_refusals = Vec::new();
    let mut aisc_tables = 0;
    for src in sources
        .iter()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        for sheet in &src.workbook.sheets {
            match locate_table(sheet, subjects) {
                Ok(None) => {}
                Err(reason) => table_refusals.push(TableRefusal {
                    file: src.file_name.clone(),
                    sheet: sheet.name.clone(),
                    reason,
                }),
                Ok(Some(table)) => {
                    aisc_tables += 1;
                    let mut explain = |subject: &str, family: IdentityFamily, miss: RateMiss| {
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
                        &mut explain,
                        &mut instances,
                        &mut unregistered,
                    );
                }
            }
        }
    }

    // The state root binds every input digest, every census and every instance's state, so changing any
    // input cell changes the root.
    let mut enc = Encoder::new_frame(*b"M0AS", 2);
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
    enc.put_u64(instances.len() as u64);
    for i in &instances {
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
    for h in &unregistered {
        enc.put_str(&format!("{}:{}!{}={}", h.file, h.sheet, h.cell, h.header));
    }
    enc.put_u64(table_refusals.len() as u64);
    for t in &table_refusals {
        enc.put_str(&format!("{}:{}:{}", t.file, t.sheet, t.reason));
    }
    let state_root = blake3_hash(&enc.finish());
    let cell_id = match lane {
        Lane::Public => format!("CELL-PUBLIC({issuer})"),
        Lane::Synthetic => "CELL-SYNTHETIC".to_string(),
    };
    let ledger_anchor = LedgerAnchor {
        format_version: FMT_ANCHOR_V1,
        cell_id,
        ledger_epoch: 1,
        commit_sequence: sources.len() as u64,
        adapter_registry_epoch: 1,
        schema_epoch: 1,
        policy_epoch: 1,
        privacy_epoch: 1,
        residency_epoch: 1,
        state_root,
    };
    let mine_stats = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .map(|s| {
            // The registry already checks the quarter format; a refusal here is kept, never dropped.
            let read =
                read_mine_stats(&s.workbook, &s.quarter).unwrap_or_else(|refusal| MineStatsRead {
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
        ledger_anchor,
    }
}

/// `fmine assure public --issuer <issuer>`: the gate-eligible run over the registered public workbooks.
pub fn run_public_assurance(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_registered_sources(profile, root)?;
    Ok(assess(profile, Lane::Public, sources))
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
    let mut sources = Vec::new();
    for (file, kind, quarter, bytes) in synthetic::synthetic_workbooks(edits) {
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
    Ok(assess(
        &synthetic::synthetic_profile(),
        Lane::Synthetic,
        sources,
    ))
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
        assert_eq!((r.aisc_tables, r.instances.len()), (1, 35));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        assert_eq!(totals, vec![21, 1, 1, 1, 6, 5]);

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
            IdentityState::Undecidable { detail } => {
                // 405000/199 = 2035.1758…: the upper bound rounds up (it printed 2035.175 before 2026-10-29).
                assert!(detail.contains("[1965.174, 2035.176]"), "{detail}")
            }
            other => panic!("D16: {other:?}"),
        }
        assert_eq!(
            state_of(&r, "E14", IdentityFamily::CosPerOunce),
            &IdentityState::Refused {
                reason: "attributable share needed"
            }
        );
        assert_eq!(state_of(&r, "E15", IdentityFamily::TccPerOunce).code(), "i");
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
            json.contains("\"totals\":{\"i\":21,\"ii\":1,\"iii\":1,\"iv\":1,\"v\":6,\"np\":5}")
        );
        assert!(json.contains("\"execution_authorized\":false"));
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
            IdentityState::Undecidable { detail } => assert!(detail.contains("B3"), "{detail}"),
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

    /// A complete run through a TOML registry in default CI (the real-corpus runs are ignored tests): the two
    /// synthetic workbooks plus a third, later Mine Stats file are written to a scratch corpus, registered by
    /// digest in a generated TOML, and loaded with no code-side profile. The ledger equals the in-code synthetic
    /// profile's, and `--through 2030Q1` drops the third file and leaves the ledger unchanged.
    #[test]
    fn test_synthetic_three_file_toml_profile_runs_end_to_end() {
        let ws = std::env::temp_dir().join(format!("fmine-public-toml-e2e-{}", std::process::id()));
        let corpus = ws.join("corpus");
        std::fs::create_dir_all(&corpus).expect("corpus dir");
        let mut files = synthetic::synthetic_workbooks(&[]);
        let later = fmine_io::xlsx::build_minimal_xlsx(&[fmine_io::xlsx::MinimalSheet {
            name: "Minestats3".to_string(),
            hidden: false,
            cells: vec![(
                "A1".to_string(),
                MinimalCell::Text("Mine Statistics (Gold)".to_string()),
            )],
        }]);
        files.push((
            "Synthetic_Q2_Mine_Stats.xlsx",
            SourceKind::MineStats,
            "2030Q2",
            later,
        ));
        let mut toml = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\n\
             retrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        for (name, kind, quarter, bytes) in &files {
            std::fs::write(corpus.join(name), bytes).expect("write workbook");
            let _ = write!(
                toml,
                "\n[[source]]\nfile = \"{name}\"\nkind = \"{}\"\nquarter = \"{quarter}\"\npath = \"{name}\"\n\
                 url_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n",
                kind.as_str(),
                bytes.len(),
                sha256(bytes).to_hex()
            );
        }
        let code_profile = synthetic::synthetic_profile();
        for s in &code_profile.subjects {
            let _ = write!(toml, "\n[[subject]]\nname = \"{s}\"\n");
        }
        let rec = &code_profile.reconstructions[0];
        let _ = write!(
            toml,
            "\n[[reconstruction]]\nevidence_id = \"{}\"\ntable_file = \"{}\"\nsheet = \"{}\"\nsubject = \"{}\"\n\
             family = \"{}\"\ncomponent_file = \"{}\"\ncomponent_sheet = \"{}\"\ncomponent_cell = \"{}\"\n\
             label_checks = \"synthetic\"\nrule = \"{}\"\n",
            rec.evidence_id,
            rec.table_file,
            rec.sheet,
            rec.subject,
            rec.family.as_str(),
            rec.component_file,
            rec.component_sheet,
            rec.component_cell,
            rec.rule
        );
        for (cell, prefix) in &rec.label_checks {
            let _ = write!(
                toml,
                "\n[[label_check]]\nset = \"synthetic\"\ncell = \"{cell}\"\nprefix = \"{prefix}\"\n"
            );
        }
        let reg = sources::registry_path(&ws, "synthetic");
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        std::fs::write(&reg, toml).expect("write registry");

        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        assert_eq!(profile.sources.len(), 3);
        assert_eq!(profile.reconstructions, code_profile.reconstructions);
        let r = run_public_assurance(&profile, &ws).expect("all three files verify by digest");
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| r.count_state(c)).collect();
        assert_eq!(
            totals,
            vec![21, 1, 1, 1, 6, 5],
            "same ledger as the in-code synthetic profile"
        );
        assert_eq!(r.sources.len(), 3);
        assert_eq!(r.ledger_anchor.commit_sequence, 3);

        let bounded =
            run_public_assurance(&profile.through("2030Q1").expect("bound"), &ws).expect("run");
        assert_eq!(bounded.sources.len(), 2);
        assert_eq!(bounded.through.as_deref(), Some("2030Q1"));
        let totals: Vec<usize> = STATE_CODES.iter().map(|c| bounded.count_state(c)).collect();
        assert_eq!(totals, vec![21, 1, 1, 1, 6, 5]);
        assert_ne!(
            bounded.ledger_anchor.state_root, r.ledger_anchor.state_root,
            "the source set is in the root"
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_next_actions_text_has_no_hard_coded_count() {
        let json = run_synthetic_assurance(&[]).expect("synthetic").to_json();
        assert!(json.contains("registries/sources/barrick.toml"), "{json}");
        assert!(!json.contains(" 18 "), "{json}");
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
            [111, 17, 379, 12]
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
                rate_consistent: 994
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
                IdentityState::Explained { component, .. } => {
                    Some(format!("{}|{}|{}", i.file, i.sheet, component))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            ii,
            vec![
                "Barrick_Q1_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10",
                "Barrick_Q2_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10",
                "Barrick_Q2_2025_AISC.xlsx|YTD Q2 2025|Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B74=13",
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

        // Regression pin (not independent; see the doc comment).
        assert_eq!(
            r.ledger_anchor.state_root.to_hex(),
            "9283f89649b8e51b63e271532d8227b801ee0e6dce522d7a1b5e60c9ad389bb2"
        );
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
