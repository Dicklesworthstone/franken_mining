//! Source registry for the public lane (`BD-055`, `BD-322`): every workbook a run reads is registered here with
//! its retrieval URL, byte size and SHA-256, and a run refuses unless the bytes on disk match.
//!
//! Provenance (FACT): `planning/research/W1_barrick_mine_stats.md` §2.1–§2.2 (E451, E452). All files were
//! downloaded from the issuer's public CDN on 2026-10-29 between 03:35:00Z and 03:36:24Z. The Mine Stats URLs
//! are W1's table; the AISC workbook URLs follow W1's stated naming rule (`Barrick_QN_YYYY_AISC.xlsx` for 2024,
//! `AISC_QN_YYYY.xlsx` from 2025, Q1 2025 in folder `2025/01/`), so their basis is INFERENCE. The digest, not the
//! URL, is what binds a run to its input.
//!
//! No affiliation: the issuer's files are public disclosures read as published.

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
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MineStats => "mine_stats",
            Self::AiscReconciliation => "aisc_reconciliation",
        }
    }
}

/// One registered public workbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredSource {
    pub file_name: &'static str,
    pub kind: SourceKind,
    /// Reporting quarter of the release, `YYYYQn`.
    pub quarter: &'static str,
    pub url: &'static str,
    /// `"W1 table"` (FACT) or `"W1 naming rule"` (INFERENCE).
    pub url_basis: &'static str,
    pub bytes: u64,
    pub sha256_hex: &'static str,
}

/// A registered explanation for one identity instance whose printed numbers close only when a component
/// published elsewhere is added (plan §21.6 state (ii), *reconstructed*: no issuer note states the rule).
/// The run re-reads the component cell, checks the labels that say what it is (row label, column header,
/// period, year), and re-does the interval arithmetic; if any check fails the instance stays undecidable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredReconstruction {
    pub evidence_id: &'static str,
    pub table_file: &'static str,
    pub sheet: &'static str,
    pub subject: &'static str,
    pub family: IdentityFamily,
    pub component_file: &'static str,
    pub component_sheet: &'static str,
    pub component_cell: &'static str,
    /// `(cell, prefix)`: the cell's text (whitespace-collapsed) must start with the prefix.
    pub label_checks: &'static [(&'static str, &'static str)],
    pub rule: &'static str,
}

/// Everything a public-lane run needs to know about one issuer: where its files live, which files, which
/// header names are reporting subjects in its AISC tables, and its registered reconstructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssuerProfile {
    pub issuer: &'static str,
    /// Directory holding the files, relative to the workspace root.
    pub corpus_dir: &'static str,
    pub retrieved_utc: &'static str,
    pub sources: &'static [RegisteredSource],
    /// Reporting subjects (sites, joint ventures, regions) as printed in AISC table headers, before the
    /// issuer's one-letter footnote suffix.
    pub subjects: &'static [&'static str],
    pub reconstructions: &'static [RegisteredReconstruction],
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

/// The 18 public Barrick workbooks of the M0 lane: Mine Stats for 2024Q1–2026Q2 (10 files) and the AISC
/// reconciliation workbooks for 2024Q3–2026Q2 (8 files; the issuer published none for 2024Q1 or 2024Q2).
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

/// The Barrick public-lane profile.
pub const BARRICK: IssuerProfile = IssuerProfile {
    issuer: "barrick",
    corpus_dir: "corpus_of_input_docs/downloaded/mine_stats",
    retrieved_utc: "2026-10-29T03:35:00Z/03:36:24Z",
    sources: &BARRICK_SOURCES,
    subjects: &BARRICK_SUBJECTS,
    reconstructions: &BARRICK_RECONSTRUCTIONS,
};

/// Looks up a registered issuer profile by its CLI name.
#[must_use]
pub fn issuer_profile(issuer: &str) -> Option<&'static IssuerProfile> {
    match issuer {
        "barrick" => Some(&BARRICK),
        _ => None,
    }
}
