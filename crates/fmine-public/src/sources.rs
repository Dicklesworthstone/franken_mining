//! Source registry for the public lane (`BD-055`, `BD-322`, bd-s9j3.1): every workbook a run reads is registered
//! with its retrieval URL, byte size and SHA-256, and a run refuses unless the bytes on disk match.
//!
//! The registry is data, read at run time from `<workspace>/registries/sources/<issuer>.toml` (schema
//! `fmine.sources.v1`, checked by `fmine_registry::sources`). Registering a new quarterly vintage is a TOML edit
//! and needs no code change or rebuild. Provenance for each issuer's rows is in the TOML file itself.
//!
//! No affiliation: the issuer's files are public disclosures read as published.

use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::sources::{
    BasisRow, SourcesRegistry, SubjectStatusRow, is_quarter, load_sources_registry,
};
pub use fmine_registry::sources::{
    DocumentRow, HtmlSourceRow, MineStatsLabelAlias, PrintedShareCorrection, RegionMemberRow,
    RestatementNoteRow, RestatementScopeRow,
};

use crate::aisc::{ExplanationBasis, IdentityFamily};
use crate::key::{StatusKind, SubjectStatus};

/// What a registered workbook is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    /// Quarterly "Mine Stats" operating workbook.
    MineStats,
    /// Quarterly reconciliation of cost of sales to total cash costs and AISC, per site and region.
    AiscReconciliation,
}

impl SourceKind {
    pub const ALL: [Self; 2] = [Self::MineStats, Self::AiscReconciliation];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MineStats => "mine_stats",
            Self::AiscReconciliation => "aisc_reconciliation",
        }
    }
}

/// One registered public workbook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSource {
    pub file_name: String,
    pub kind: SourceKind,
    /// Reporting quarter of the release, `YYYYQn`.
    pub quarter: String,
    pub url: String,
    /// `"W1 table"` (FACT), `"W1 naming rule"` (INFERENCE) or `"fetched and verified YYYY-MM-DD"` (FACT: the
    /// URL served the registered bytes on that date).
    pub url_basis: String,
    pub bytes: u64,
    pub sha256_hex: String,
}

/// A registered explanation for one identity instance whose printed numbers close only when a component
/// published elsewhere is added (plan §21.6 state (ii)). `basis` says whether an issuer note states the rule.
/// The run re-reads the component cell, checks the labels that say what it is (row label, column header,
/// period, year), re-reads a stated rule's quote, and re-does the interval arithmetic; if any check fails the
/// instance stays undecidable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredReconstruction {
    pub evidence_id: String,
    pub table_file: String,
    pub sheet: String,
    pub subject: String,
    pub family: IdentityFamily,
    pub component_file: String,
    pub component_sheet: String,
    pub component_cell: String,
    /// `(cell, prefix)`: the cell's text (whitespace-collapsed) must start with the prefix.
    pub label_checks: Vec<(String, String)>,
    pub rule: String,
    pub basis: ExplanationBasis,
}

/// The fact that would decide a (iv) instance of `family` for `subject` (BD-331.S2): prose with no digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredMissingFact {
    pub subject: String,
    pub family: IdentityFamily,
    pub text: String,
    pub evidence: String,
    pub closing_document: Option<String>,
}

/// The issuer's stated ownership share in a site (BD-331.S3), used for the per-ounce rates the issuer states on
/// an attributable basis. The run uses it only after the same-release Mine Stats label prints the same share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredShare {
    pub subject: String,
    pub num: i64,
    pub den: i64,
    pub evidence: String,
    /// The Mine Stats label text before `(x%)`; the subject name unless registered otherwise.
    pub label: String,
}

/// Everything a public-lane run needs to know about one issuer: where its files live, which files, which
/// header names are reporting subjects in its AISC tables, and its registered reconstructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerProfile {
    pub issuer: String,
    /// Directory holding the files, relative to the workspace root.
    pub corpus_dir: String,
    pub retrieved_utc: String,
    pub sources: Vec<RegisteredSource>,
    /// Reporting subjects (sites, joint ventures, regions) as printed in AISC table headers, before the
    /// issuer's one-letter footnote suffix.
    pub subjects: Vec<String>,
    /// Subjects that are regions (segment roll-ups of sites at different shares).
    pub regions: Vec<String>,
    /// Registered stated shares, at most one per site.
    pub shares: Vec<RegisteredShare>,
    pub reconstructions: Vec<RegisteredReconstruction>,
    /// The registered Mine Statistics label vocabulary as `(section, label)`. When it is non-empty, a data row
    /// whose label is not in it is refused by locator (bd-gwf1.1); when empty, labels are read unchecked.
    pub mine_stats_labels: Vec<(Option<String>, String)>,
    /// Registered missing facts, at most one per `(subject, family)`.
    pub missing_facts: Vec<RegisteredMissingFact>,
    /// Registered subject statuses (bd-a28f.1), at most one per site.
    pub subject_statuses: Vec<SubjectStatus>,
    /// The region map (BD-331.S4): which sites each region rolls up, by release quarter.
    pub region_members: Vec<RegionMemberRow>,
    /// Printed shares the Mine Stats ledger replaces with a registered one (BD-331.S4, W1-DR-01).
    pub printed_share_corrections: Vec<PrintedShareCorrection>,
    /// Stale Mine Statistics row labels the ledger reads as registered ones (BD-331.S4, W1-DR-02).
    pub mine_stats_label_aliases: Vec<MineStatsLabelAlias>,
    /// Registered PDF documents (bd-plx1). A run reads none of their values; it checks the bytes of those its
    /// transcription registry cites ([`crate::transcribe`]).
    pub documents: Vec<DocumentRow>,
    /// Registered HTML filings read directly by the label-bound canonical Claim adapter.
    pub html_sources: Vec<HtmlSourceRow>,
    /// Explicit, source-bound disclosure notes and their exact semantic coverage. Runtime verification
    /// re-reads each quote before a comparison can classify a change as disclosed.
    pub restatement_notes: Vec<RestatementNoteRow>,
    pub restatement_scopes: Vec<RestatementScopeRow>,
    /// The `--through YYYYQn` bound applied, if any; later rows were dropped before loading.
    pub through: Option<String>,
}

impl IssuerProfile {
    /// Converts a checked registry into a profile. Fails only if the registry's kind or family names drift
    /// from this crate's enums (a test pins the two lists together).
    pub fn from_registry(reg: SourcesRegistry) -> Result<Self, String> {
        let kind = |name: &str| {
            SourceKind::ALL
                .into_iter()
                .find(|k| k.as_str() == name)
                .ok_or_else(|| format!("source kind {name:?} has no SourceKind"))
        };
        let family = |name: &str| {
            IdentityFamily::ALL
                .into_iter()
                .find(|f| f.as_str() == name)
                .ok_or_else(|| format!("family {name:?} has no IdentityFamily"))
        };
        let mut sources = Vec::with_capacity(reg.sources.len());
        for s in reg.sources {
            sources.push(RegisteredSource {
                kind: kind(&s.kind)?,
                file_name: s.file,
                quarter: s.quarter,
                url: s.url,
                url_basis: s.url_basis,
                bytes: s.bytes,
                sha256_hex: s.sha256,
            });
        }
        let mut reconstructions = Vec::with_capacity(reg.reconstructions.len());
        for r in reg.reconstructions {
            reconstructions.push(RegisteredReconstruction {
                family: family(&r.family)?,
                evidence_id: r.evidence_id,
                table_file: r.table_file,
                sheet: r.sheet,
                subject: r.subject,
                component_file: r.component_file,
                component_sheet: r.component_sheet,
                component_cell: r.component_cell,
                label_checks: r.label_checks,
                rule: r.rule,
                basis: match r.basis {
                    BasisRow::Reconstructed => ExplanationBasis::Reconstructed,
                    BasisRow::Stated {
                        doc,
                        locator,
                        quote_prefix,
                    } => ExplanationBasis::Stated {
                        doc,
                        locator,
                        quote_prefix,
                    },
                },
            });
        }
        let mut missing_facts = Vec::with_capacity(reg.missing_facts.len());
        for m in reg.missing_facts {
            missing_facts.push(RegisteredMissingFact {
                family: family(&m.family)?,
                subject: m.subject,
                text: m.text,
                evidence: m.evidence,
                closing_document: m.closing_document,
            });
        }
        let mut shares = Vec::with_capacity(reg.shares.len());
        for s in reg.shares {
            let (Ok(num), Ok(den)) = (i64::try_from(s.num), i64::try_from(s.den)) else {
                return Err(format!(
                    "share {}/{} of {} is out of range",
                    s.num, s.den, s.subject
                ));
            };
            shares.push(RegisteredShare {
                label: s.label.unwrap_or_else(|| s.subject.clone()),
                subject: s.subject,
                num,
                den,
                evidence: s.evidence,
            });
        }
        Ok(Self {
            issuer: reg.issuer,
            corpus_dir: reg.corpus_dir,
            retrieved_utc: reg.retrieved_utc,
            sources,
            subjects: reg.subjects,
            regions: reg.regions,
            shares,
            reconstructions,
            mine_stats_labels: reg
                .mine_stats_labels
                .into_iter()
                .map(|r| (r.section, r.label))
                .collect(),
            missing_facts,
            subject_statuses: reg
                .subject_statuses
                .iter()
                .map(subject_status)
                .collect::<Result<_, _>>()?,
            region_members: reg.region_members,
            printed_share_corrections: reg.printed_share_corrections,
            mine_stats_label_aliases: reg.mine_stats_label_aliases,
            documents: reg.documents,
            html_sources: reg.html_sources,
            restatement_notes: reg.restatement_notes,
            restatement_scopes: reg.restatement_scopes,
            through: None,
        })
    }

    /// The registered members of `region` in a file released in `release` (`YYYYQn`), in registry order.
    #[must_use]
    pub fn members_of(&self, region: &str, release: &str) -> Vec<String> {
        self.region_members
            .iter()
            .filter(|m| m.region == region && m.holds_for(release))
            .map(|m| m.member.clone())
            .collect()
    }

    /// Keeps only the sources whose quarter is at or before `bound` (`YYYYQn`, already validated), so a later
    /// registry reproduces an earlier run. Refuses when no source remains (`FMINE-REF-201`: no registered
    /// workbook covers the requested periods).
    pub fn through(mut self, bound: &str) -> Result<Self, Refusal> {
        self.sources.retain(|s| s.quarter.as_str() <= bound);
        if self.sources.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "no registered {} workbooks at or before {bound}",
                    self.issuer
                ),
            ));
        }
        self.restatement_notes.retain(|note| {
            note.from_quarter.as_str() <= bound
                && self.sources.iter().any(|s| s.file_name == note.file_name)
        });
        self.restatement_scopes.retain(|scope| {
            self.restatement_notes
                .iter()
                .any(|note| note.id == scope.note)
        });
        self.through = Some(bound.to_string());
        Ok(self)
    }
}

/// Converts a checked `[[subject_status]]` row: a disposal covers the months after its closing date; any other
/// status covers its first quarter's first month through its last quarter's last month.
fn subject_status(row: &SubjectStatusRow) -> Result<SubjectStatus, String> {
    let drift = || format!("subject status row for {} does not convert", row.subject);
    let int = |s: &str| s.parse::<u16>().map_err(|_| drift());
    let quarter = |q: &str, last: bool| -> Result<(u16, u8), String> {
        let (y, n) = q.split_once('Q').ok_or_else(drift)?;
        let n = u8::try_from(int(n)?).map_err(|_| drift())?;
        Ok((int(y)?, if last { 3 * n } else { 3 * n - 2 }))
    };
    let kind = match row.status.as_str() {
        "disposed" => StatusKind::Disposed,
        "withheld" => StatusKind::Withheld,
        "care_and_maintenance" => StatusKind::CareAndMaintenance,
        other => return Err(format!("status {other:?} has no StatusKind")),
    };
    if let Some(date) = &row.closed {
        let mut parts = date.split('-');
        let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(drift());
        };
        let byte = |s: &str| int(s).and_then(|v| u8::try_from(v).map_err(|_| drift()));
        return Ok(SubjectStatus::from_date(
            &row.subject,
            kind,
            (int(y)?, byte(m)?, byte(d)?),
            &row.evidence,
        ));
    }
    let first = row.first.as_deref().ok_or_else(drift)?;
    Ok(SubjectStatus {
        subject: row.subject.clone(),
        kind,
        first: quarter(first, false)?,
        last: row.last.as_deref().map(|q| quarter(q, true)).transpose()?,
        evidence: row.evidence.clone(),
    })
}

/// One row of `repro/<issuer>/expected.toml` (BD-339.S1): what a run of the registered files printed at one
/// code revision. The file is append-only; the last row is the current expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedRoot {
    pub commit: String,
    /// sha256 over the sorted `"{file}\t{sha256}\n"` lines of the registry rows the run used.
    pub registry_epoch: String,
    pub state_root: String,
    pub anchor_id: String,
    /// Instance counts in the order i, ii, iii, iv, v, np.
    pub totals: [u64; 6],
    /// sha256 over `"{file}\t{numeric_digest}\n"` in registry order.
    pub census_digest_of_digests: String,
    pub reason: String,
    pub evidence: String,
    /// 1-based line of the row's `[[expected]]` header.
    pub line: usize,
}

/// `<workspace>/repro/<issuer>/expected.toml`.
#[must_use]
pub fn expected_roots_path(workspace: &Path, issuer: &str) -> PathBuf {
    workspace.join("repro").join(issuer).join("expected.toml")
}

/// Reads the `[[expected]]` rows of an expected-roots file, in file order. Refuses an unknown table or key, a
/// missing key, a value of the wrong type, and a file with no rows; the message names the line.
pub fn load_expected_roots(path: &Path) -> Result<Vec<ExpectedRoot>, String> {
    use fmine_registry::toml_subset::{TomlValue, parse_subset};
    const KEYS: [&str; 13] = [
        "commit",
        "registry_epoch",
        "state_root",
        "anchor_id",
        "totals_i",
        "totals_ii",
        "totals_iii",
        "totals_iv",
        "totals_v",
        "totals_np",
        "census_digest_of_digests",
        "reason",
        "evidence",
    ];
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc = parse_subset(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for row in &doc.rows {
        if row.table != "expected" {
            return Err(format!(
                "line {}: unknown table [[{}]]",
                row.line, row.table
            ));
        }
        if let Some(f) = row.fields.iter().find(|f| !KEYS.contains(&f.key.as_str())) {
            return Err(format!("line {}: unknown key {:?}", f.line, f.key));
        }
        let s = |key: &str| match row.get(key).map(|f| &f.value) {
            Some(TomlValue::Str(v)) => Ok(v.clone()),
            _ => Err(format!("line {}: {key} must be a string", row.line)),
        };
        let n = |key: &str| match row.get(key).map(|f| &f.value) {
            Some(TomlValue::Int(v)) => u64::try_from(*v)
                .map_err(|_| format!("line {}: {key} must not be negative", row.line)),
            _ => Err(format!("line {}: {key} must be an integer", row.line)),
        };
        rows.push(ExpectedRoot {
            commit: s("commit")?,
            registry_epoch: s("registry_epoch")?,
            state_root: s("state_root")?,
            anchor_id: s("anchor_id")?,
            totals: [
                n("totals_i")?,
                n("totals_ii")?,
                n("totals_iii")?,
                n("totals_iv")?,
                n("totals_v")?,
                n("totals_np")?,
            ],
            census_digest_of_digests: s("census_digest_of_digests")?,
            reason: s("reason")?,
            evidence: s("evidence")?,
            line: row.line,
        });
    }
    if rows.is_empty() {
        return Err(format!("{}: no [[expected]] rows", path.display()));
    }
    Ok(rows)
}

/// Markers around the generated section of `repro/<issuer>/README.md`.
pub const KIT_SELECTORS_BEGIN: &str = "<!-- BEGIN GENERATED SELECTORS -->";
pub const KIT_SELECTORS_END: &str = "<!-- END GENERATED SELECTORS -->";

/// The selectors section of `repro/<issuer>/README.md` (BD-339.S4). It is rendered from the code's row anchors
/// and the issuer's registry, so the README cannot drift from either: the row anchors, the registered subjects,
/// the footnote rule, the reconstructions with their label checks, and one row per registered file. It holds
/// no value read from a workbook.
#[must_use]
pub fn kit_selectors_markdown(profile: &IssuerProfile) -> String {
    use crate::aisc::{AnchorRule, ExplanationBasis, ROW_ANCHORS};
    let code = |s: &str| format!("`{s}`");
    let mut out = String::from(
        "### Row anchors\n\n\
         A sheet's column-A labels are read lower-cased with whitespace collapsed. Each row below is the first \
         label, top to bottom, that matches one of its labels. A sheet with no cost-of-sales row holds no AISC \
         table; one that has it but lacks another row is refused as a table, naming the row.\n\n\
         | Row | Labels | Match |\n|---|---|---|\n",
    );
    for a in ROW_ANCHORS {
        let names: Vec<String> = a.names.iter().map(|n| code(n)).collect();
        let rule = match a.rule {
            AnchorRule::Exact => "equals",
            AnchorRule::Prefix => "starts with",
        };
        out.push_str(&format!("| {} | {} | {rule} |\n", a.role, names.join(", ")));
    }
    out.push_str(
        "\n### Subjects\n\n\
         A column header names a subject when it equals a registered subject, or is the subject followed by one \
         lower-case footnote letter. Any other header is reported as unregistered. Regions are segment \
         roll-ups.\n\n",
    );
    for s in &profile.subjects {
        let region = if profile.regions.contains(s) {
            " (region)"
        } else {
            ""
        };
        out.push_str(&format!("- {s}{region}\n"));
    }
    if !profile.region_members.is_empty() {
        out.push_str(
            "\n### Region members\n\n\
             A region's cost-of-sales rate, where its sheet states an attributable basis, is checked against the \
             members registered for the table's release, weighted by attributable ounces sold. A member the sheet \
             does not print is named as absent; no last release means the row still holds.\n\n\
             | Region | Member | First release | Last release |\n|---|---|---|---|\n",
        );
        for m in &profile.region_members {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                m.region,
                m.member,
                m.first,
                m.last.as_deref().unwrap_or("")
            ));
        }
    }
    out.push_str(
        "\n### Reconstructions\n\n\
         An identity that closes only when a component published elsewhere is added. The run re-reads the \
         component cell and checks that each listed cell's text starts with the listed prefix; if any check \
         fails, the instance stays undecidable.\n\n\
         | Evidence | Table | Subject | Family | Component | Label checks | Basis |\n|---|---|---|---|---|---|---|\n",
    );
    for r in &profile.reconstructions {
        let checks: Vec<String> = r
            .label_checks
            .iter()
            .map(|(cell, prefix)| format!("{} starts {}", code(cell), code(prefix)))
            .collect();
        let basis = match &r.basis {
            ExplanationBasis::Reconstructed => "reconstructed".to_string(),
            ExplanationBasis::Stated { doc, locator, .. } => {
                format!("stated at {}", code(&format!("{doc}:{locator}")))
            }
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {basis} |\n",
            r.evidence_id,
            code(&format!("{} {}", r.table_file, r.sheet)),
            r.subject,
            r.family.as_str(),
            code(&format!(
                "{} {}!{}",
                r.component_file, r.component_sheet, r.component_cell
            )),
            checks.join("; "),
        ));
    }
    out.push_str(
        "\n### Registered files\n\n\
         | File | Kind | Quarter | URL | URL basis | Bytes | SHA-256 |\n|---|---|---|---|---|---|---|\n",
    );
    for s in &profile.sources {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            code(&s.file_name),
            s.kind.as_str(),
            s.quarter,
            s.url,
            s.url_basis,
            s.bytes,
            code(&s.sha256_hex)
        ));
    }
    out
}

/// The text between the generated-section markers of a kit README, or `None` if either marker is missing.
#[must_use]
pub fn kit_readme_section(readme: &str) -> Option<&str> {
    let start = readme.find(KIT_SELECTORS_BEGIN)? + KIT_SELECTORS_BEGIN.len();
    let len = readme[start..].find(KIT_SELECTORS_END)?;
    Some(readme[start..start + len].trim_matches('\n'))
}

/// An issuer name as used on the command line and as a registry file stem.
#[must_use]
pub fn is_issuer_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

/// Whether `s` is a valid `--through` bound (`YYYYQ1` … `YYYYQ4`).
#[must_use]
pub fn is_through_bound(s: &str) -> bool {
    is_quarter(s)
}

/// `<workspace>/registries/sources/<issuer>.toml`.
#[must_use]
pub fn registry_path(workspace: &Path, issuer: &str) -> PathBuf {
    workspace
        .join("registries")
        .join("sources")
        .join(format!("{issuer}.toml"))
}

/// `<workspace>/registries/transcriptions/<issuer>.toml` (bd-plx1). Git-ignored: real rows are held back pending
/// OPEN-R7-1.
#[must_use]
pub fn transcriptions_path(workspace: &Path, issuer: &str) -> PathBuf {
    workspace
        .join("registries")
        .join("transcriptions")
        .join(format!("{issuer}.toml"))
}

/// The issuers with a registry file under `<workspace>/registries/sources/`, sorted.
#[must_use]
pub fn registered_issuers(workspace: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(workspace.join("registries").join("sources"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter_map(|e| {
                    let name = e.file_name().into_string().ok()?;
                    let stem = name.strip_suffix(".toml")?;
                    is_issuer_name(stem).then(|| stem.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Loads and checks the issuer's registry from `<workspace>/registries/sources/<issuer>.toml`. A missing or
/// malformed file refuses (`FMINE-REF-506`, registered input missing or unreadable; exit 5) naming the file
/// and the line.
pub fn load_issuer_profile(workspace: &Path, issuer: &str) -> Result<IssuerProfile, Refusal> {
    if !is_issuer_name(issuer) {
        return Err(crate::input_refusal(format!(
            "issuer {issuer:?} is not a registry name"
        )));
    }
    let path = registry_path(workspace, issuer);
    let bad = |e: String| crate::input_refusal(format!("source registry {}: {e}", path.display()));
    let reg = load_sources_registry(&path).map_err(|e| bad(e.to_string()))?;
    IssuerProfile::from_registry(reg).map_err(|e| bad(e.to_string()))
}

// The source-registry contract tests run through this public API in xtask/tests/public_registry.rs.
