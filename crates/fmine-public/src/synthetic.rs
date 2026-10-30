//! Synthetic lane: workbooks in the real AISC-table layout with invented subjects ("Site A" …) and invented
//! numbers, each column seeded with a known answer. A synthetic run prints `lane = synthetic`, is never
//! eligible for `GATE-BULL-01`, and exists so every state of the ledger is exercised without any issuer's
//! bytes (`BD-321`).
//!
//! Seeded answers (row numbers as below):
//! - B "Site A": every identity consistent.
//! - C "Site Bb" (footnote letter): TCC prints 350 against a component sum of 250; flipping the sign of C6
//!   (−50) closes it, so the certificate names C6 as the witness.
//! - D "Site C": AISC per ounce prints 2400 where 1000·(200 ± 5/2)/(100 ± 1/2) allows only
//!   [1965.174, 2035.176] (undecidable; bounds rounded outward).
//! - E "Site D": a non-zero non-controlling-interest line, so cost of sales per ounce is refused.
//! - F "Site E": every used cell zero or a dash (not published).
//! - G "Site F": stored values 100.4 and 80.4 under `#,##0`, so the ledger takes the displayed 100 and 80
//!   (bd-jg43.3): TCC 100 − 20 + 0 = 80 and AISC 80 + 2 + 18 = 100 foot, and the rates fit their intervals
//!   (cost of sales [1970.297, 2030.303] holds 2008, TCC with k = 3 [1554.455, 1646.465] holds 1608, AISC with
//!   k = 5 [1930.693, 2070.707] holds 2008).
//! - N "Site G": a General non-integer at N6 and a date format (id 14) at N13, so the TCC footing and all three
//!   rates are refused by cell and format code; its AISC footing (rows 8 to 11) foots.
//! - H "Region North": AISC per ounce 2830 closes only with the registered component in the synthetic
//!   Mine Stats workbook, `Minestats3!B32` = 13 (explained).
//! - I "Footnote" is skipped; J "Mystery Mine" is not a registered subject.
//! - K "Site H" and L "Site I": a non-zero non-controlling-interest line and a registered share of 3/4 that the
//!   synthetic Mine Stats `QTD` sheet confirms. The stated share rule (footnote `A30`, BD-331.S3) gives
//!   cost of sales per ounce 1000·¾·400/150 = 2000: K prints 2000 (consistent at the stated share), L prints 2200,
//!   whose feasible share interval [0.8210, 0.8290] excludes 3/4 (undecidable). Site D has no registered share.
//! - M "Site J": AISC per ounce 2830 closes with the same component as H, and the footnote at `A31` states the
//!   rule, so the explanation is *stated* (BD-331.S2); H's is *reconstructed*.
//! - D's (iv) names Site C's registered missing fact; L's names the stated share rule's.
//! - The hidden sheet `Q4 2029` holds one table: B "Site A" prints TCC 401 against a component sum of 400
//!   (issuer discrepancy, hidden scope only). The hidden sheet `Scratch` holds A1 = 1 and no table.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;
use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx, build_minimal_xlsx_styled,
};

use crate::aisc::{ExplanationBasis, IdentityFamily};
use crate::sources::{
    IssuerProfile, RegisteredMissingFact, RegisteredReconstruction, RegisteredShare, SourceKind,
    registry_path,
};

/// The issuer name reserved for the synthetic lane: a run under it is never gate-eligible.
pub const SYNTHETIC_ISSUER: &str = "synthetic";
pub const SYNTHETIC_AISC_FILE: &str = "Synthetic_AISC.xlsx";
pub const SYNTHETIC_STATS_FILE: &str = "Synthetic_Mine_Stats.xlsx";
pub const SYNTHETIC_TABLE_SHEET: &str = "Q1 2030";
/// The hidden sheet holding a table (BD-331.S2).
pub const SYNTHETIC_HIDDEN_SHEET: &str = "Q4 2029";

/// The issuer note stating Site J's rule, at `A31` of the table sheet.
pub const SYNTHETIC_STATED_QUOTE: &str = "i.  Site J's all-in sustaining costs per ounce include Site G's \
minesite sustaining capital.";

pub const SYNTHETIC_SUBJECTS: [&str; 11] = [
    "Site A",
    "Site B",
    "Site C",
    "Site D",
    "Site E",
    "Site F",
    "Site G",
    "Region North",
    "Site H",
    "Site I",
    "Site J",
];

/// Registered shares of the synthetic lane `(subject, num, den)`, printed as `(75%)` on the `QTD` sheet.
pub const SYNTHETIC_SHARES: [(&str, i64, i64); 2] = [("Site H", 3, 4), ("Site I", 3, 4)];

/// The synthetic statement of the share rule, with a footnote letter the real files never use at this cell.
pub const SYNTHETIC_SHARE_FOOTNOTE: &str = "h.  Cost of sales per ounce: Gold cost of sales per ounce is \
calculated as cost of sales divided by ounces sold (both on an attributable basis using the issuer's ownership share).";

/// The synthetic lane's profile: its subjects, shares, two registered reconstructions (one reconstructed, one
/// stated) and one missing fact. It has no registered sources because the synthetic workbooks are built in memory
/// (`synthetic_workbooks`), never read from disk.
#[must_use]
pub fn synthetic_profile() -> IssuerProfile {
    let s = |x: &str| x.to_string();
    let reconstruction =
        |evidence_id: &str, subject: &str, rule: &str, basis| RegisteredReconstruction {
            evidence_id: s(evidence_id),
            table_file: s(SYNTHETIC_AISC_FILE),
            sheet: s(SYNTHETIC_TABLE_SHEET),
            subject: s(subject),
            family: IdentityFamily::AiscPerOunce,
            component_file: s(SYNTHETIC_STATS_FILE),
            component_sheet: s("Minestats3"),
            component_cell: s("B32"),
            label_checks: [
                ("A32", "Minesite sustaining"),
                ("B2", "Site G"),
                ("A3", "For the three months ended March 31"),
                ("B3", "2030"),
            ]
            .iter()
            .map(|(c, p)| (s(c), s(p)))
            .collect(),
            rule: s(rule),
            basis,
        };
    IssuerProfile {
        issuer: s(SYNTHETIC_ISSUER),
        corpus_dir: String::new(),
        retrieved_utc: String::new(),
        sources: Vec::new(),
        subjects: SYNTHETIC_SUBJECTS.iter().map(|x| s(x)).collect(),
        regions: vec![s("Region North")],
        shares: SYNTHETIC_SHARES
            .iter()
            .map(|&(subject, num, den)| RegisteredShare {
                subject: s(subject),
                num,
                den,
                evidence: s("synthetic"),
                label: s(subject),
            })
            .collect(),
        reconstructions: vec![
            reconstruction(
                "SYNTHETIC",
                "Region North",
                "Synthetic: the region's AISC numerator includes Site G's minesite sustaining capital.",
                ExplanationBasis::Reconstructed,
            ),
            reconstruction(
                "SYNTHETIC-STATED",
                "Site J",
                "Synthetic: Site J's AISC numerator includes Site G's minesite sustaining capital, as A31 states.",
                ExplanationBasis::Stated {
                    doc: s(SYNTHETIC_AISC_FILE),
                    locator: format!("{SYNTHETIC_TABLE_SHEET}!A31"),
                    // The run collapses whitespace before comparing, as for label checks.
                    quote_prefix: s("i. Site J's all-in sustaining costs per ounce include"),
                },
            ),
        ],
        mine_stats_labels: Vec::new(),
        missing_facts: vec![RegisteredMissingFact {
            subject: s("Site C"),
            family: IdentityFamily::AiscPerOunce,
            text: s("the sustaining capital the synthetic issuer used for this rate"),
            evidence: s("synthetic"),
            closing_document: None,
        }],
        subject_statuses: Vec::new(),
        region_members: Vec::new(),
        printed_share_corrections: Vec::new(),
        mine_stats_label_aliases: Vec::new(),
        through: None,
    }
}

fn n(r: &str, v: f64) -> (String, MinimalCell) {
    (r.to_string(), MinimalCell::Number(v))
}

fn t(r: &str, s: &str) -> (String, MinimalCell) {
    (r.to_string(), MinimalCell::Text(s.to_string()))
}

/// The cell styles of the synthetic AISC workbook: 0 General, 1 `#,##0` (built-in 3), 2 `m/d/yyyy` (built-in 14).
#[must_use]
pub fn synthetic_aisc_styles() -> MinimalStyles {
    MinimalStyles {
        num_fmts: Vec::new(),
        xfs: vec![0, 3, 14],
    }
}

fn styled(r: &str, raw: &str, xf: u32) -> (String, MinimalCell) {
    (
        r.to_string(),
        MinimalCell::Styled {
            raw: raw.to_string(),
            xf,
        },
    )
}

/// Cells of the synthetic AISC table sheet.
#[must_use]
pub fn synthetic_aisc_cells() -> Vec<(String, MinimalCell)> {
    let mut cells = vec![
        t(
            "A1",
            "Reconciliation of cost of sales to total cash costs and all-in sustaining costs",
        ),
        t("A3", "($ millions, except per ounce information)"),
        t("A5", "Cost of sales applicable to gold production"),
        t("A6", "Depreciation"),
        t("A7", "Non-controlling interests"),
        t("A8", "Total cash costs"),
        t("A9", "General & administrative costs"),
        t("A10", "Minesite sustaining capital expenditures"),
        t("A11", "All-in sustaining costs"),
        t("A13", "Ounces sold - attributable basis (000s ounces)"),
        t("A14", "Cost of sales per ounce"),
        t("A15", "Total cash costs per ounce"),
        t("A16", "All-in sustaining costs per ounce"),
        t("I2", "Footnote"),
        t("I5", "a"),
        t("J2", "Mystery Mine"),
        n("J5", 1.0),
        (String::from("B20"), MinimalCell::Error("#REF!".to_string())),
        t("A30", SYNTHETIC_SHARE_FOOTNOTE),
        t("A31", SYNTHETIC_STATED_QUOTE),
    ];
    let headers = [
        ("B", "Site A"),
        ("C", "Site Bb"),
        ("D", "Site C"),
        ("E", "Site D"),
        ("F", "Site E"),
        ("G", "Site F"),
        ("H", "Region North"),
        ("K", "Site H"),
        ("L", "Site I"),
        ("M", "Site J"),
        ("N", "Site G"),
    ];
    for (col, h) in headers {
        cells.push(t(&format!("{col}2"), h));
    }
    // Rows 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16.
    let rows = [5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16];
    let columns: [(&str, [f64; 11]); 8] = [
        (
            "B",
            [
                500.0, -100.0, 0.0, 400.0, 10.0, 90.0, 500.0, 400.0, 1250.0, 1000.0, 1250.0,
            ],
        ),
        (
            "C",
            [
                300.0, -50.0, 0.0, 350.0, 20.0, 30.0, 400.0, 200.0, 1500.0, 1750.0, 2000.0,
            ],
        ),
        (
            "D",
            [
                200.0, -40.0, 0.0, 160.0, 5.0, 35.0, 200.0, 100.0, 2000.0, 1600.0, 2400.0,
            ],
        ),
        (
            "E",
            [
                1000.0, -200.0, -160.0, 640.0, 10.0, 150.0, 800.0, 400.0, 2500.0, 1600.0, 2000.0,
            ],
        ),
        (
            "H",
            [
                300.0, -60.0, 0.0, 240.0, 0.0, 30.0, 270.0, 100.0, 3000.0, 2400.0, 2830.0,
            ],
        ),
        (
            "K",
            [
                400.0, -100.0, -75.0, 225.0, 10.0, 15.0, 250.0, 150.0, 2000.0, 1500.0, 1667.0,
            ],
        ),
        (
            "L",
            [
                400.0, -100.0, -75.0, 225.0, 10.0, 15.0, 250.0, 150.0, 2200.0, 1500.0, 1667.0,
            ],
        ),
        (
            "M",
            [
                300.0, -60.0, 0.0, 240.0, 0.0, 30.0, 270.0, 100.0, 3000.0, 2400.0, 2830.0,
            ],
        ),
    ];
    for (col, values) in columns {
        for (row, v) in rows.iter().zip(values) {
            cells.push(n(&format!("{col}{row}"), v));
        }
    }
    // Stored text and cell style (`synthetic_aisc_styles`) of the two formatted columns.
    let formatted: [(&str, [(&str, u32); 11]); 2] = [
        (
            "G",
            [
                ("100.4", 1),
                ("-20", 1),
                ("0", 1),
                ("80.4", 1),
                ("2", 1),
                ("18", 1),
                ("100.4", 1),
                ("50", 1),
                ("2008", 1),
                ("1608", 1),
                ("2008", 1),
            ],
        ),
        (
            "N",
            [
                ("300", 0),
                ("-50.5", 0),
                ("0", 0),
                ("250", 0),
                ("10", 0),
                ("40", 0),
                ("300", 0),
                ("45000", 2),
                ("2000", 0),
                ("1667", 0),
                ("2000", 0),
            ],
        ),
    ];
    for (col, values) in formatted {
        for (row, (raw, xf)) in rows.iter().zip(values) {
            cells.push(styled(&format!("{col}{row}"), raw, xf));
        }
    }
    for row in rows {
        let r = format!("F{row}");
        cells.push(if matches!(row, 6 | 7 | 9 | 10) {
            t(&r, "-")
        } else {
            n(&r, 0.0)
        });
    }
    cells
}

/// Cells of the hidden `Q4 2029` table: Site A prints TCC 401 against lines summing to 400 (one (iii)); its AISC,
/// 501, foots on the printed TCC, and its three rates fit.
#[must_use]
pub fn synthetic_hidden_cells() -> Vec<(String, MinimalCell)> {
    let mut cells: Vec<(String, MinimalCell)> = synthetic_aisc_cells()
        .into_iter()
        .filter(|(r, _)| {
            (r.starts_with('A') && !matches!(r.as_str(), "A30" | "A31")) || r.as_str() == "B2"
        })
        .collect();
    let rows = [5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16];
    let values = [
        500.0, -100.0, 0.0, 401.0, 10.0, 90.0, 501.0, 400.0, 1250.0, 1003.0, 1253.0,
    ];
    for (row, v) in rows.iter().zip(values) {
        cells.push(n(&format!("B{row}"), v));
    }
    cells
}

/// Cells of the synthetic Mine Stats `QTD` sheet: the share labels in column A, Site H's as given (tests pass a
/// different printed share to break the confirmation).
#[must_use]
pub fn synthetic_share_label_cells(site_h_label: &str) -> Vec<(String, MinimalCell)> {
    vec![
        t("A1", "Summary of Operations"),
        t("A3", "For the three months ended March 31,"),
        n("B3", 2030.0),
        t("A5", site_h_label),
        t("A6", "Site I (75%)"),
    ]
}

/// The synthetic Mine Stats workbook: the `QTD` share labels (Site H's as given) and the `Minestats3` component.
#[must_use]
pub fn synthetic_stats_workbook(site_h_label: &str) -> Vec<u8> {
    build_minimal_xlsx(&[
        MinimalSheet {
            name: "QTD".to_string(),
            hidden: false,
            cells: synthetic_share_label_cells(site_h_label),
        },
        MinimalSheet {
            name: "Minestats3".to_string(),
            hidden: false,
            cells: vec![
                t("A1", "Mine Statistics (Gold)"),
                t("B2", "Site G (60%)1"),
                t("A3", "For the three months ended March 31,"),
                t("B3", "2030"),
                t("A32", "Minesite sustaining3"),
                n("B32", 13.0),
            ],
        },
    ])
}

/// Builds the synthetic workbooks; `edits` replace or add cells of the AISC table sheet (for perturbation
/// tests). Returns `(file name, kind, quarter, bytes)`.
#[must_use]
pub fn synthetic_workbooks(
    edits: &[(String, MinimalCell)],
) -> Vec<(&'static str, SourceKind, &'static str, Vec<u8>)> {
    let mut cells = synthetic_aisc_cells();
    for (r, c) in edits {
        cells.retain(|(existing, _)| existing != r);
        cells.push((r.clone(), c.clone()));
    }
    let aisc = build_minimal_xlsx_styled(
        &[
            MinimalSheet {
                name: SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells,
            },
            MinimalSheet {
                name: "Scratch".to_string(),
                hidden: true,
                cells: vec![n("A1", 1.0)],
            },
            MinimalSheet {
                name: SYNTHETIC_HIDDEN_SHEET.to_string(),
                hidden: true,
                cells: synthetic_hidden_cells(),
            },
        ],
        Some(&synthetic_aisc_styles()),
    );
    let stats = synthetic_stats_workbook("Site H (75%)1");
    vec![
        (SYNTHETIC_STATS_FILE, SourceKind::MineStats, "2030Q1", stats),
        (
            SYNTHETIC_AISC_FILE,
            SourceKind::AiscReconciliation,
            "2030Q1",
            aisc,
        ),
    ]
}

/// A later synthetic Mine Stats vintage (2030Q2) with a title and nothing else.
pub const SYNTHETIC_LATER_STATS_FILE: &str = "Synthetic_Q2_Mine_Stats.xlsx";

/// Writes a synthetic issuer workspace under `ws`, so the registry-and-digest path of
/// `fmine assure public --issuer synthetic --workspace <ws>` runs end to end on invented numbers. The issuer
/// name is reserved, so the run is in the synthetic lane and never gate-eligible.
///
/// It writes the two synthetic workbooks plus a later, title-only Mine Stats vintage (2030Q2) into
/// `ws/corpus`, and `registries/sources/synthetic.toml`, which registers all three by size and sha256 along with
/// the synthetic subjects, the reconstruction and its label checks. Returns the registry path.
///
/// # Errors
/// The I/O error of the first directory or file that cannot be written.
pub fn write_issuer_workspace(ws: &Path) -> std::io::Result<PathBuf> {
    let corpus = ws.join("corpus");
    std::fs::create_dir_all(&corpus)?;
    let mut files = synthetic_workbooks(&[]);
    let later = build_minimal_xlsx(&[MinimalSheet {
        name: "Minestats3".to_string(),
        hidden: false,
        cells: vec![t("A1", "Mine Statistics (Gold)")],
    }]);
    files.push((
        SYNTHETIC_LATER_STATS_FILE,
        SourceKind::MineStats,
        "2030Q2",
        later,
    ));
    let mut toml = String::from(
        "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\n\
         retrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n",
    );
    for (name, kind, quarter, bytes) in &files {
        std::fs::write(corpus.join(name), bytes)?;
        let _ = write!(
            toml,
            "\n[[source]]\nfile = \"{name}\"\nkind = \"{}\"\nquarter = \"{quarter}\"\npath = \"{name}\"\n\
             url_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n",
            kind.as_str(),
            bytes.len(),
            sha256(bytes).to_hex()
        );
    }
    let profile = synthetic_profile();
    for s in &profile.subjects {
        let kind = if profile.regions.contains(s) {
            "kind = \"region\"\n"
        } else {
            ""
        };
        let _ = write!(toml, "\n[[subject]]\nname = \"{s}\"\n{kind}");
    }
    for s in &profile.shares {
        let _ = write!(
            toml,
            "\n[[share]]\nsubject = \"{}\"\nnum = {}\nden = {}\nevidence = \"{}\"\n",
            s.subject, s.num, s.den, s.evidence
        );
    }
    for m in &profile.missing_facts {
        let _ = write!(
            toml,
            "\n[[missing_fact]]\nsubject = \"{}\"\nfamily = \"{}\"\ntext = \"{}\"\nevidence = \"{}\"\n",
            m.subject,
            m.family.as_str(),
            m.text,
            m.evidence
        );
    }
    for rec in &profile.reconstructions {
        let basis = match &rec.basis {
            ExplanationBasis::Reconstructed => "basis = \"reconstructed\"\n".to_string(),
            ExplanationBasis::Stated {
                doc,
                locator,
                quote_prefix,
            } => format!(
                "basis = \"stated\"\nquote_doc = \"{doc}\"\nquote_locator = \"{locator}\"\n\
                 quote_prefix = \"{quote_prefix}\"\n"
            ),
        };
        let _ = write!(
            toml,
            "\n[[reconstruction]]\nevidence_id = \"{}\"\ntable_file = \"{}\"\nsheet = \"{}\"\nsubject = \"{}\"\n\
             family = \"{}\"\ncomponent_file = \"{}\"\ncomponent_sheet = \"{}\"\ncomponent_cell = \"{}\"\n\
             label_checks = \"{}\"\nrule = \"{}\"\n{basis}",
            rec.evidence_id,
            rec.table_file,
            rec.sheet,
            rec.subject,
            rec.family.as_str(),
            rec.component_file,
            rec.component_sheet,
            rec.component_cell,
            rec.evidence_id,
            rec.rule
        );
        for (cell, prefix) in &rec.label_checks {
            let _ = write!(
                toml,
                "\n[[label_check]]\nset = \"{}\"\ncell = \"{cell}\"\nprefix = \"{prefix}\"\n",
                rec.evidence_id
            );
        }
    }
    let reg = registry_path(ws, SYNTHETIC_ISSUER);
    if let Some(dir) = reg.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&reg, toml)?;
    Ok(reg)
}
