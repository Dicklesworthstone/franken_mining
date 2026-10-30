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
use fmine_registry::sources::{SourcesRegistry, is_quarter, load_sources_registry};

use crate::aisc::IdentityFamily;

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
/// published elsewhere is added (plan §21.6 state (ii), *reconstructed*: no issuer note states the rule).
/// The run re-reads the component cell, checks the labels that say what it is (row label, column header,
/// period, year), and re-does the interval arithmetic; if any check fails the instance stays undecidable.
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
    pub reconstructions: Vec<RegisteredReconstruction>,
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
            });
        }
        Ok(Self {
            issuer: reg.issuer,
            corpus_dir: reg.corpus_dir,
            retrieved_utc: reg.retrieved_utc,
            sources,
            subjects: reg.subjects,
            reconstructions,
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
                format!("no registered {} workbooks at or before {bound}", self.issuer),
            ));
        }
        self.through = Some(bound.to_string());
        Ok(self)
    }
}

/// An issuer name as used on the command line and as a registry file stem.
#[must_use]
pub fn is_issuer_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
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
        Refusal::from_code("FMINE-REF-502", format!("source registry {}: {e}", path.display()))
    })?;
    IssuerProfile::from_registry(reg).map_err(|e| {
        Refusal::from_code("FMINE-REF-502", format!("source registry {}: {e}", path.display()))
    })
}

/// The registry constants this module held before bd-s9j3.1, kept only for the golden test that proves the
/// TOML registry carries the same rows. Deleted once that test passes (the bead's two-commit plan).
#[cfg(test)]
pub(crate) mod former {
    use super::SourceKind;
    use crate::aisc::IdentityFamily;

    pub struct RegisteredSource {
        pub file_name: &'static str,
        pub kind: SourceKind,
        pub quarter: &'static str,
        pub url: &'static str,
        pub url_basis: &'static str,
        pub bytes: u64,
        pub sha256_hex: &'static str,
    }

    pub struct RegisteredReconstruction {
        pub evidence_id: &'static str,
        pub table_file: &'static str,
        pub sheet: &'static str,
        pub subject: &'static str,
        pub family: IdentityFamily,
        pub component_file: &'static str,
        pub component_sheet: &'static str,
        pub component_cell: &'static str,
        pub label_checks: &'static [(&'static str, &'static str)],
        pub rule: &'static str,
    }

macro_rules! src {
    ($file:literal, $kind:ident, $q:literal, $path:literal, $basis:literal, $bytes:literal, $sha:literal) => {
        RegisteredSource {
            file_name: $file,
            kind: SourceKind::$kind,
            quarter: $q,
            url: concat!(
                "https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/",
                $path
            ),
            url_basis: $basis,
            bytes: $bytes,
            sha256_hex: $sha,
        }
    };
}

pub const BARRICK_SOURCES: [RegisteredSource; 18] = [
    src!(
        "Barrick_Q1_2024_Mine_Stats.xlsx",
        MineStats,
        "2024Q1",
        "2024/q1/Barrick_Q1_2024_Mine_Stats.xlsx",
        "W1 table",
        43_668,
        "e2e1243ec0549f4c487c773869876e2acade401a0b6cd1141adf4b9296aec840"
    ),
    src!(
        "Barrick_Q2_2024_Mine_Stats.xlsx",
        MineStats,
        "2024Q2",
        "2024/q2/Barrick_Q2_2024_Mine_Stats.xlsx",
        "W1 table",
        109_832,
        "1a73d6d7d4658f923757e873eec147ccc74aa8aada6a439f3c98da8540ff68d5"
    ),
    src!(
        "Barrick_Q3_2024_Mine_Stats.xlsx",
        MineStats,
        "2024Q3",
        "2024/q3/Barrick_Q3_2024_Mine_Stats.xlsx",
        "W1 table",
        110_573,
        "d8786ce96f7129bf4cc240e01d2fe7840bfde0df4082517975578e69d73ead3b"
    ),
    src!(
        "Barrick_Q4_2024_Mine_Stats.xlsx",
        MineStats,
        "2024Q4",
        "2024/q4/Barrick_Q4_2024_Mine_Stats.xlsx",
        "W1 table",
        110_614,
        "f5e7db7c4657927f96e8684b40f9b50c0061fa2e0c22912aa480babdb49ef4cd"
    ),
    src!(
        "Barrick_Q1_2025_Mine_Stats.xlsx",
        MineStats,
        "2025Q1",
        "2025/01/Barrick_Q1_2025_Mine_Stats.xlsx",
        "W1 table",
        45_695,
        "ec5158f8b4624643ee83d0099e4f3354dc550729ebb443a2dbfacfc29315998b"
    ),
    src!(
        "Barrick_Q2_2025_Mine_Stats.xlsx",
        MineStats,
        "2025Q2",
        "2025/q2/Barrick_Q2_2025_Mine_Stats.xlsx",
        "W1 table",
        107_135,
        "599864f9958b5a109530c321e9481fa7d59c53ed2c730be96f51035390a2d822"
    ),
    src!(
        "Barrick_Q3_2025_Mine_Stats.xlsx",
        MineStats,
        "2025Q3",
        "2025/q3/Barrick_Q3_2025_Mine_Stats.xlsx",
        "W1 table",
        108_497,
        "b55b857766195fe90464cf813e7a71a5112439d606a6fc96ec3b6c5f01b58715"
    ),
    src!(
        "Barrick_Q4_2025_Mine_Stats.xlsx",
        MineStats,
        "2025Q4",
        "2025/q4/Barrick_Q4_2025_Mine_Stats.xlsx",
        "W1 table",
        117_396,
        "94f6692acfa5f133b97ae028f172cc9c5e7527945788da7bd9a06ffd11c7eec4"
    ),
    src!(
        "Barrick_Q1_2026_Mine_Stats.xlsx",
        MineStats,
        "2026Q1",
        "2026/q1/Barrick_Q1_2026_Mine_Stats.xlsx",
        "W1 table",
        55_644,
        "ddef08dd544f0df511fc30f53c1dd889bb6878257d394b8baad7ac3dbfa8c5d2"
    ),
    src!(
        "Barrick_Q2_2026_Mine_Stats.xlsx",
        MineStats,
        "2026Q2",
        "2026/q2/Barrick_Q2_2026_Mine_Stats.xlsx",
        "W1 table",
        131_296,
        "cf0d3f4fea9c96f967c138d0aca549f4ff4f4a233bd2c5946c4035bf96c3df8e"
    ),
    src!(
        "Barrick_Q3_2024_AISC.xlsx",
        AiscReconciliation,
        "2024Q3",
        "2024/q3/Barrick_Q3_2024_AISC.xlsx",
        "W1 naming rule",
        40_977,
        "d063ea7c1cd667a6db9c9de1b3aa4cb0ff2dba4a8cb24f6ce889e56af2fd0ef7"
    ),
    src!(
        "Barrick_Q4_2024_AISC.xlsx",
        AiscReconciliation,
        "2024Q4",
        "2024/q4/Barrick_Q4_2024_AISC.xlsx",
        "W1 naming rule",
        42_141,
        "f842696915b7a44d00194e01a3bdf4a76d1e52d0e31c2610a91be70afb4b1b4d"
    ),
    src!(
        "Barrick_Q1_2025_AISC.xlsx",
        AiscReconciliation,
        "2025Q1",
        "2025/01/AISC_Q1_2025.xlsx",
        "W1 naming rule",
        32_587,
        "06951434c3a3e325553dbe25018544c7329bc1a8b6e01ab0d3fcdeba2bb4388e"
    ),
    src!(
        "Barrick_Q2_2025_AISC.xlsx",
        AiscReconciliation,
        "2025Q2",
        "2025/q2/AISC_Q2_2025.xlsx",
        "W1 naming rule",
        41_599,
        "1cbd6b739a276d64e4752ebbc67c369720ccb8eec16845a4858dccd04fd6d178"
    ),
    src!(
        "Barrick_Q3_2025_AISC.xlsx",
        AiscReconciliation,
        "2025Q3",
        "2025/q3/AISC_Q3_2025.xlsx",
        "W1 naming rule",
        41_020,
        "62901aaab21d64057422a950e29e0ad2ecebad525e0c2b8b9185e4843d9ecfd3"
    ),
    src!(
        "Barrick_Q4_2025_AISC.xlsx",
        AiscReconciliation,
        "2025Q4",
        "2025/q4/AISC_Q4_2025.xlsx",
        "W1 naming rule",
        40_336,
        "2ce7b8e46b36d745228487e683b221101c4db3019ba67acedea96967486d6755"
    ),
    src!(
        "Barrick_Q1_2026_AISC.xlsx",
        AiscReconciliation,
        "2026Q1",
        "2026/q1/AISC_Q1_2026.xlsx",
        "W1 naming rule",
        43_718,
        "dbe567e90730b4ef87330b1eaa539c7289a92781e452b949c24f7b705464ef28"
    ),
    src!(
        "Barrick_Q2_2026_AISC.xlsx",
        AiscReconciliation,
        "2026Q2",
        "2026/q2/AISC_Q2_2026.xlsx",
        "W1 naming rule",
        49_738,
        "66515a4ae4305de130b72fa1a3a78bf6aa51ac5a10350fc43732bb621c6d0e99"
    ),
];

/// Reporting subjects printed in the issuer's AISC table headers (FACT: every header of the 34 visible AISC
/// tables in the eight registered workbooks matches exactly one of these, with or without a footnote letter).
pub const BARRICK_SUBJECTS: [&str; 18] = [
    "Nevada Gold Mines",
    "Carlin",
    "Cortez",
    "Turquoise Ridge",
    "Phoenix",
    "Long Canyon",
    "Hemlo",
    "Pueblo Viejo",
    "Veladero",
    "Porgera",
    "Loulo-Gounkoto",
    "Kibali",
    "North Mara",
    "Tongon",
    "Bulyanhulu",
    "North America",
    "Latin America & Asia Pacific",
    "Africa & Middle East",
];

const E816_RULE: &str = "The region's per-ounce AISC numerator includes Loulo-Gounkoto's minesite sustaining \
capital although the site's $M rows and ounces print nil; no issuer note states the rule (E816).";

/// Q1 2025 Mine Stats, sheet `Minestats3`, three-month block (rows 2–43): FACT, read 2026-10-29.
const E816_Q1_2025_CHECKS: &[(&str, &str)] = &[
    ("A32", "Minesite sustaining"),
    ("B2", "Loulo-Gounkoto"),
    ("A3", "For the three months ended March 31"),
    ("B3", "2025"),
];

/// Q2 2025 Mine Stats, sheet `Minestats3`, six-month block (header row 44): FACT, read 2026-10-29.
const E816_H1_2025_CHECKS: &[(&str, &str)] = &[
    ("A74", "Minesite sustaining"),
    ("B44", "Loulo-Gounkoto"),
    ("A45", "For the six months ended June 30"),
    ("B45", "2025"),
];

/// E816: Africa & Middle East AISC per ounce, Q1 2025 (two vintages) and H1 2025.
pub const BARRICK_RECONSTRUCTIONS: [RegisteredReconstruction; 3] = [
    RegisteredReconstruction {
        evidence_id: "E816",
        table_file: "Barrick_Q1_2025_AISC.xlsx",
        sheet: "Q1 2025",
        subject: "Africa & Middle East",
        family: IdentityFamily::AiscPerOunce,
        component_file: "Barrick_Q1_2025_Mine_Stats.xlsx",
        component_sheet: "Minestats3",
        component_cell: "B32",
        label_checks: E816_Q1_2025_CHECKS,
        rule: E816_RULE,
    },
    RegisteredReconstruction {
        evidence_id: "E816",
        table_file: "Barrick_Q2_2025_AISC.xlsx",
        sheet: "Q1 2025",
        subject: "Africa & Middle East",
        family: IdentityFamily::AiscPerOunce,
        component_file: "Barrick_Q1_2025_Mine_Stats.xlsx",
        component_sheet: "Minestats3",
        component_cell: "B32",
        label_checks: E816_Q1_2025_CHECKS,
        rule: E816_RULE,
    },
    RegisteredReconstruction {
        evidence_id: "E816",
        table_file: "Barrick_Q2_2025_AISC.xlsx",
        sheet: "YTD Q2 2025",
        subject: "Africa & Middle East",
        family: IdentityFamily::AiscPerOunce,
        component_file: "Barrick_Q2_2025_Mine_Stats.xlsx",
        component_sheet: "Minestats3",
        component_cell: "B74",
        label_checks: E816_H1_2025_CHECKS,
        rule: E816_RULE,
    },
];

pub const CORPUS_DIR: &str = "corpus_of_input_docs/downloaded/mine_stats";
pub const RETRIEVED_UTC: &str = "2026-10-29T03:35:00Z/03:36:24Z";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    /// Golden: `registries/sources/barrick.toml` carries exactly the rows the Rust constants held, field by
    /// field and in the same order (the root binds sources in registry order).
    #[test]
    fn test_toml_registry_equals_former_constants() {
        let p = load_issuer_profile(&workspace_root(), "barrick").expect("barrick registry");
        assert_eq!(p.issuer, "barrick");
        assert_eq!(p.corpus_dir, former::CORPUS_DIR);
        assert_eq!(p.retrieved_utc, former::RETRIEVED_UTC);
        assert_eq!(p.through, None);
        assert_eq!(p.sources.len(), former::BARRICK_SOURCES.len());
        for (got, want) in p.sources.iter().zip(former::BARRICK_SOURCES.iter()) {
            assert_eq!(
                (
                    got.file_name.as_str(),
                    got.kind,
                    got.quarter.as_str(),
                    got.url.as_str(),
                    got.url_basis.as_str(),
                    got.bytes,
                    got.sha256_hex.as_str()
                ),
                (
                    want.file_name,
                    want.kind,
                    want.quarter,
                    want.url,
                    want.url_basis,
                    want.bytes,
                    want.sha256_hex
                )
            );
        }
        assert_eq!(p.subjects, former::BARRICK_SUBJECTS);
        assert_eq!(p.reconstructions.len(), former::BARRICK_RECONSTRUCTIONS.len());
        for (got, want) in p.reconstructions.iter().zip(former::BARRICK_RECONSTRUCTIONS.iter()) {
            assert_eq!(
                (
                    got.evidence_id.as_str(),
                    got.table_file.as_str(),
                    got.sheet.as_str(),
                    got.subject.as_str(),
                    got.family,
                    got.component_file.as_str(),
                    got.component_sheet.as_str(),
                    got.component_cell.as_str(),
                    got.rule.as_str()
                ),
                (
                    want.evidence_id,
                    want.table_file,
                    want.sheet,
                    want.subject,
                    want.family,
                    want.component_file,
                    want.component_sheet,
                    want.component_cell,
                    want.rule
                )
            );
            let checks: Vec<(&str, &str)> =
                got.label_checks.iter().map(|(c, p)| (c.as_str(), p.as_str())).collect();
            assert_eq!(checks, want.label_checks);
        }
    }

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
        assert!(h1_2025.sources.iter().all(|s| s.quarter.as_str() <= "2025Q2"));
        assert!(h1_2025.sources.iter().any(|s| s.quarter == "2025Q2"));
        // Order is preserved: the filtered rows are a prefix-respecting subsequence of the registry.
        let names: Vec<&str> = h1_2025.sources.iter().map(|s| s.file_name.as_str()).collect();
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
        assert!(is_through_bound("2026Q2") && !is_through_bound("2026Q5") && !is_through_bound("2026-Q2"));
    }

    #[test]
    fn test_missing_or_malformed_registry_refuses_naming_file_and_line() {
        let dir = std::env::temp_dir().join(format!("fmine-public-registry-{}", std::process::id()));
        let sources = dir.join("registries").join("sources");
        std::fs::create_dir_all(&sources).expect("temp registry dir");
        let e = load_issuer_profile(&dir, "nobody").expect_err("no file");
        assert_eq!(e.code, "FMINE-REF-502");
        assert!(e.what.contains("nobody.toml"), "{}", e.what);
        let text = std::fs::read_to_string(registry_path(&workspace_root(), "barrick")).expect("read");
        let broken = text.replacen("bytes = 43_668", "bytes = 43.668", 1);
        let line = broken.lines().position(|l| l == "bytes = 43.668").map(|i| i + 1).expect("edited");
        std::fs::write(sources.join("barrick.toml"), broken).expect("write");
        let e = load_issuer_profile(&dir, "barrick").expect_err("float refused");
        assert!(e.what.contains(&format!("line {line}: ")) && e.what.contains("floats"), "{}", e.what);
        assert!(load_issuer_profile(&dir, "../etc").is_err());
        assert_eq!(registered_issuers(&workspace_root()), vec!["barrick".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
