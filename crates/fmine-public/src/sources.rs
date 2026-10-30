//! Source registry for the public lane (`BD-055`, `BD-322`, bd-s9j3.1): every workbook a run reads is registered
//! with its retrieval URL, byte size and SHA-256, and a run refuses unless the bytes on disk match.
//!
//! The registry is data, read at run time from `<workspace>/registries/sources/<issuer>.toml` (schema
//! `fmine.sources.v1`, checked by `fmine_registry::sources`). Registering a new quarterly vintage is a TOML edit
//! and needs no code change or rebuild. Provenance for each issuer's rows is in the TOML file itself.
//!
//! No affiliation: the issuer's files are public disclosures read as published.

use std::path::{Path, PathBuf};

use fmine_evidence::Refusal;
use fmine_registry::sources::{BasisRow, SourcesRegistry, is_quarter, load_sources_registry};

use crate::aisc::{ExplanationBasis, IdentityFamily};

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
    /// `"W1 table"` (FACT) or `"W1 naming rule"` (INFERENCE).
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
            through: None,
        })
    }

    /// Keeps only the sources whose quarter is at or before `bound` (`YYYYQn`, already validated), so a later
    /// registry reproduces an earlier run. Refuses when no source remains.
    pub fn through(mut self, bound: &str) -> Result<Self, Refusal> {
        self.sources.retain(|s| s.quarter.as_str() <= bound);
        if self.sources.is_empty() {
            return Err(Refusal::from_code(
                "FMINE-REF-502",
                format!(
                    "no registered {} workbooks at or before {bound}",
                    self.issuer
                ),
            ));
        }
        self.through = Some(bound.to_string());
        Ok(self)
    }
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
/// malformed file refuses (`FMINE-REF-502`; BD-328 owns the final code) naming the file and the line.
pub fn load_issuer_profile(workspace: &Path, issuer: &str) -> Result<IssuerProfile, Refusal> {
    if !is_issuer_name(issuer) {
        return Err(Refusal::from_code(
            "FMINE-REF-502",
            format!("issuer {issuer:?} is not a registry name"),
        ));
    }
    let path = registry_path(workspace, issuer);
    let reg = load_sources_registry(&path).map_err(|e| {
        Refusal::from_code(
            "FMINE-REF-502",
            format!("source registry {}: {e}", path.display()),
        )
    })?;
    IssuerProfile::from_registry(reg).map_err(|e| {
        Refusal::from_code(
            "FMINE-REF-502",
            format!("source registry {}: {e}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    // The field-by-field golden test against the former Rust constants passed at commit 997b65e and was removed
    // with them. What still binds the registry: registry-check (xtask), the rows' digests checked on every load,
    // and the real-corpus root pinned in `barrick_real_files_match_the_independent_reader`.

    /// The registry crate's name lists must match this crate's enums, or a checked registry could fail to map.
    #[test]
    fn test_registry_name_lists_match_enums() {
        let kinds: Vec<&str> = SourceKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(kinds, fmine_registry::sources::SOURCE_KINDS);
        let families: Vec<&str> = IdentityFamily::ALL.iter().map(|f| f.as_str()).collect();
        assert_eq!(families, fmine_registry::sources::IDENTITY_FAMILIES);
    }

    #[test]
    fn test_through_filters_by_quarter() {
        let p = load_issuer_profile(&workspace_root(), "barrick").expect("barrick registry");
        let all = p.sources.len();
        let h1_2025 = p.clone().through("2025Q2").expect("rows remain");
        assert_eq!(h1_2025.through.as_deref(), Some("2025Q2"));
        assert!(h1_2025.sources.len() < all);
        assert!(
            h1_2025
                .sources
                .iter()
                .all(|s| s.quarter.as_str() <= "2025Q2")
        );
        assert!(h1_2025.sources.iter().any(|s| s.quarter == "2025Q2"));
        // Order is preserved: the filtered rows are a prefix-respecting subsequence of the registry.
        let names: Vec<&str> = h1_2025
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        let kept: Vec<&str> = p
            .sources
            .iter()
            .filter(|s| s.quarter.as_str() <= "2025Q2")
            .map(|s| s.file_name.as_str())
            .collect();
        assert_eq!(names, kept);
        assert_eq!(p.clone().through("2099Q4").expect("all").sources.len(), all);
        let e = p.through("2023Q4").expect_err("nothing that early");
        assert_eq!(e.code, "FMINE-REF-502");
        assert!(
            is_through_bound("2026Q2")
                && !is_through_bound("2026Q5")
                && !is_through_bound("2026-Q2")
        );
    }

    #[test]
    fn test_missing_or_malformed_registry_refuses_naming_file_and_line() {
        let dir =
            std::env::temp_dir().join(format!("fmine-public-registry-{}", std::process::id()));
        let sources = dir.join("registries").join("sources");
        std::fs::create_dir_all(&sources).expect("temp registry dir");
        let e = load_issuer_profile(&dir, "nobody").expect_err("no file");
        assert_eq!(e.code, "FMINE-REF-502");
        assert!(e.what.contains("nobody.toml"), "{}", e.what);
        let text =
            std::fs::read_to_string(registry_path(&workspace_root(), "barrick")).expect("read");
        let broken = text.replacen("bytes = 43_668", "bytes = 43.668", 1);
        let line = broken
            .lines()
            .position(|l| l == "bytes = 43.668")
            .map(|i| i + 1)
            .expect("edited");
        std::fs::write(sources.join("barrick.toml"), broken).expect("write");
        let e = load_issuer_profile(&dir, "barrick").expect_err("float refused");
        assert!(
            e.what.contains(&format!("line {line}: ")) && e.what.contains("floats"),
            "{}",
            e.what
        );
        assert!(load_issuer_profile(&dir, "../etc").is_err());
        assert_eq!(
            registered_issuers(&workspace_root()),
            vec!["barrick".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
