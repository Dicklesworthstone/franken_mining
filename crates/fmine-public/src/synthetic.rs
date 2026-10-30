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
//! - G "Site F": a stored value of 100.4 (refused: unrounded).
//! - H "Region North": AISC per ounce 2830 closes only with the registered component in the synthetic
//!   Mine Stats workbook, `Minestats3!B32` = 13 (explained).
//! - I "Footnote" is skipped; J "Mystery Mine" is not a registered subject.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};

use crate::aisc::IdentityFamily;
use crate::sources::{IssuerProfile, RegisteredReconstruction, SourceKind, registry_path};

/// The issuer name reserved for the synthetic lane: a run under it is never gate-eligible.
pub const SYNTHETIC_ISSUER: &str = "synthetic";
pub const SYNTHETIC_AISC_FILE: &str = "Synthetic_AISC.xlsx";
pub const SYNTHETIC_STATS_FILE: &str = "Synthetic_Mine_Stats.xlsx";
pub const SYNTHETIC_TABLE_SHEET: &str = "Q1 2030";

pub const SYNTHETIC_SUBJECTS: [&str; 8] = [
    "Site A",
    "Site B",
    "Site C",
    "Site D",
    "Site E",
    "Site F",
    "Site G",
    "Region North",
];

/// The synthetic lane's profile: its subjects and its one registered reconstruction. It has no registered
/// sources because the synthetic workbooks are built in memory (`synthetic_workbooks`), never read from disk.
#[must_use]
pub fn synthetic_profile() -> IssuerProfile {
    let s = |x: &str| x.to_string();
    IssuerProfile {
        issuer: s(SYNTHETIC_ISSUER),
        corpus_dir: String::new(),
        retrieved_utc: String::new(),
        sources: Vec::new(),
        subjects: SYNTHETIC_SUBJECTS.iter().map(|x| s(x)).collect(),
        reconstructions: vec![RegisteredReconstruction {
            evidence_id: s("SYNTHETIC"),
            table_file: s(SYNTHETIC_AISC_FILE),
            sheet: s(SYNTHETIC_TABLE_SHEET),
            subject: s("Region North"),
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
            rule: s(
                "Synthetic: the region's AISC numerator includes Site G's minesite sustaining capital.",
            ),
        }],
        mine_stats_labels: Vec::new(),
        through: None,
    }
}

fn n(r: &str, v: f64) -> (String, MinimalCell) {
    (r.to_string(), MinimalCell::Number(v))
}

fn t(r: &str, s: &str) -> (String, MinimalCell) {
    (r.to_string(), MinimalCell::Text(s.to_string()))
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
    ];
    let headers = [
        ("B", "Site A"),
        ("C", "Site Bb"),
        ("D", "Site C"),
        ("E", "Site D"),
        ("F", "Site E"),
        ("G", "Site F"),
        ("H", "Region North"),
    ];
    for (col, h) in headers {
        cells.push(t(&format!("{col}2"), h));
    }
    // Rows 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16.
    let rows = [5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16];
    let columns: [(&str, [f64; 11]); 6] = [
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
            "G",
            [
                100.4, -20.0, 0.0, 80.4, 2.0, 18.0, 100.4, 50.0, 2008.0, 1608.0, 2008.0,
            ],
        ),
        (
            "H",
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
    let aisc = build_minimal_xlsx(&[
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
    ]);
    let stats = build_minimal_xlsx(&[MinimalSheet {
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
    }]);
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
        let _ = write!(toml, "\n[[subject]]\nname = \"{s}\"\n");
    }
    for rec in &profile.reconstructions {
        let _ = write!(
            toml,
            "\n[[reconstruction]]\nevidence_id = \"{}\"\ntable_file = \"{}\"\nsheet = \"{}\"\nsubject = \"{}\"\n\
             family = \"{}\"\ncomponent_file = \"{}\"\ncomponent_sheet = \"{}\"\ncomponent_cell = \"{}\"\n\
             label_checks = \"{}\"\nrule = \"{}\"\n",
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
