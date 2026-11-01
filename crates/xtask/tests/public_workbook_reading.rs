//! Cross-crate workbook reading checks through the public APIs.
//! Fixtures and assertions are relocated from `fmine-public`; private parser tests remain there.
//! Public assurance display/refusal and mutation checks are retained in `public_report_contract.rs`.

#[cfg(test)]
mod mine_stats_oracle {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    use fmine_core::hash::{Digest32, sha256};
    use fmine_io::xlsx::{CellValue, Sheet};
    use fmine_public::mine_stats::{LabelVocabulary, MineStatsCell, is_m0_target, read_mine_stats};
    use fmine_public::sources::SourceKind;
    use fmine_public::{load_issuer_profile, load_registered_sources};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// `s` as a JSON string literal.
    fn json_str(s: &str) -> String {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// One JSON line per cell where two sets of canonical lines differ, keyed by sheet and cell, with both reads.
    /// The token (a number's bits, or the stored placeholder) is never printed, so no issuer value reaches a log
    /// (RISK-31); `differs` names the fields that disagree, `token` among them. A cell one read lacks, or reads
    /// more than once, differs in `presence`.
    fn mismatch_lines(file: &str, rust: &[String], oracle: &[String]) -> Vec<String> {
        const NAMES: [&str; 8] = [
            "view", "period", "site_raw", "site", "label", "section", "kind", "token",
        ];
        type Cells = BTreeMap<(String, String), Vec<Vec<String>>>;
        let by_cell = |lines: &[String]| {
            let mut out = Cells::new();
            for line in lines {
                let mut fields = line.split('\t').map(str::to_string);
                let sheet = fields.next().unwrap_or_default();
                let cell = fields.next().unwrap_or_default();
                out.entry((sheet, cell)).or_default().push(fields.collect());
            }
            out
        };
        let (rust, oracle) = (by_cell(rust), by_cell(oracle));
        let shown = |read: Option<&Vec<Vec<String>>>| match read {
            None => "null".to_string(),
            Some(entries) => {
                let objects: Vec<String> = entries
                    .iter()
                    .map(|f| {
                        let pairs: Vec<String> = NAMES[..7]
                            .iter()
                            .zip(f)
                            .map(|(n, v)| format!("\"{n}\":{}", json_str(v)))
                            .collect();
                        format!("{{{}}}", pairs.join(","))
                    })
                    .collect();
                format!("[{}]", objects.join(","))
            }
        };
        let keys: BTreeSet<&(String, String)> = rust.keys().chain(oracle.keys()).collect();
        let mut out = Vec::new();
        for key in keys {
            let (r, o) = (rust.get(key), oracle.get(key));
            if r == o {
                continue;
            }
            let differs: Vec<String> = match (r.map(Vec::as_slice), o.map(Vec::as_slice)) {
                (Some([a]), Some([b])) => NAMES
                    .iter()
                    .enumerate()
                    .filter(|&(k, _)| a.get(k) != b.get(k))
                    .map(|(_, n)| json_str(n))
                    .collect(),
                _ => vec![json_str("presence")],
            };
            out.push(format!(
                "{{\"mismatch\":{},\"cell\":{},\"rust\":{},\"oracle\":{},\"differs\":[{}]}}",
                json_str(file),
                json_str(&format!("{}!{}", key.0, key.1)),
                shown(r),
                shown(o),
                differs.join(",")
            ));
        }
        out
    }

    /// The per-mismatch log of the real-file test: each differing cell is named with both reads, the field that
    /// differs is named, and no token is printed.
    #[test]
    fn test_mismatch_lines_name_the_cell_and_never_the_token() {
        let line = |cell: &str, site: &str, token: &str| {
            format!(
                "Minestats1\t{cell}\tminestats\t2030Q2\t{site}\t{site}\tProcessed\t-\tnumber\t{token}"
            )
        };
        let rust = vec![
            line("B4", "Site A", "4059000000000000"),
            line("B5", "Site A", "4059000000000000"),
            line("C4", "Site B", "4024000000000000"),
        ];
        let oracle = vec![
            line("B4", "Site A", "4059000000000000"),
            line("B5", "Site A", "405a000000000000"),
            line("C4", "Site C", "4024000000000000"),
            line("D4", "Site D", "4024000000000000"),
        ];
        assert!(mismatch_lines("f.xlsx", &rust, &rust).is_empty());
        let out = mismatch_lines("f.xlsx", &rust, &oracle);
        assert_eq!(out.len(), 3, "{out:#?}");
        assert!(out[0].contains("\"cell\":\"Minestats1!B5\""), "{}", out[0]);
        assert!(out[0].ends_with("\"differs\":[\"token\"]}"), "{}", out[0]);
        assert!(out[1].contains("\"cell\":\"Minestats1!C4\""), "{}", out[1]);
        assert!(
            out[1].ends_with("\"differs\":[\"site_raw\",\"site\"]}"),
            "{}",
            out[1]
        );
        assert!(out[1].contains("\"site\":\"Site B\"") && out[1].contains("\"site\":\"Site C\""));
        assert!(out[2].contains("\"rust\":null"), "{}", out[2]);
        assert!(
            out[2].ends_with("\"differs\":[\"presence\"]}"),
            "{}",
            out[2]
        );
        assert!(
            out.iter()
                .all(|l| !l.contains("4059") && !l.contains("405a") && !l.contains("4024")),
            "a token reached the log"
        );
        assert_eq!(json_str("a\"b\\c\n"), "\"a\\\"b\\\\c\\u000a\"");
    }

    /// The first four characters of a period code are its year; the same period one year later.
    fn year_later(period: &str) -> String {
        match period.get(..4).and_then(|y| y.parse::<u16>().ok()) {
            Some(year) => format!("{}{}", year + 1, &period[4..]),
            None => period.to_string(),
        }
    }

    fn lines_digest(mut lines: Vec<String>) -> String {
        lines.sort();
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        sha256(text.as_bytes()).to_hex()
    }

    /// The deliberately wrong fixed-coordinate reader uses raw public XLSX cells. It preserves the
    /// reference reader's whitespace and placeholder rules while retaining last year's row coordinates.
    fn typed_cell(sheet: &Sheet, row: u32, col: u32) -> Result<MineStatsCell, String> {
        let Some(cell) = sheet.cell(row, col) else {
            return Ok(MineStatsCell::Missing);
        };
        match &cell.value {
            CellValue::Number(v) => Ok(MineStatsCell::Number {
                raw_text: cell.number_text.clone().unwrap_or_else(|| v.to_string()),
                value: *v,
            }),
            CellValue::Placeholder(t) | CellValue::Text(t) => match t
                .trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
            {
                "—" => Ok(MineStatsCell::Nil { token: t.clone() }),
                "n/a" => Ok(MineStatsCell::NotApplicable { token: t.clone() }),
                _ => Err(format!(
                    "token {t:?} is outside the set (a number, —, n/a, blank)"
                )),
            },
            CellValue::FormulaError(t) => Err(format!("formula error {t:?} in a data cell")),
        }
    }

    /// GATE-BULL-01 part 1 on the ten registered Mine Stats workbooks. Every expected count and digest comes
    /// from W1's openpyxl reader (`scripts/oracle/mine_stats_extract3.py`, run 2026-10-29 before this code; its
    /// output `long_all3.json` sha256 8992ae50…81dd), turned into canonical lines by
    /// `scripts/oracle/mine_stats_lines.py`. None of it comes from this implementation.
    ///
    /// `[cells, number, numeric zero, nil, na, missing]`; "number" includes the zeros. Set
    /// `FMINE_MINE_STATS_DUMP=<dir>` to write this reader's lines per file for a diff against the oracle's. Set
    /// `FMINE_MINE_STATS_ORACLE=<dir>` to the oracle's per-file lines (`mine_stats_lines.py`'s dump,
    /// `Q1_2024.oracle.tsv` and so on) to log one JSON line per differing cell, with both reads and no token,
    /// before the digests are compared.
    #[test]
    #[ignore = "needs the registered Barrick Mine Stats workbooks under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_mine_stats_match_the_w1_reader() {
        let oracle: &[(&str, [usize; 6], &str)] = &[
            (
                "Barrick_Q1_2024_Mine_Stats.xlsx",
                [1380, 1082, 0, 108, 190, 0],
                "4e513f4c0c7eca8bd2a4a43a3c66356a4d8b6f873b0d558e54d6c1ac767411ec",
            ),
            (
                "Barrick_Q2_2024_Mine_Stats.xlsx",
                [2760, 2204, 0, 183, 373, 0],
                "c5241d4c2cd55bc1ea982c98ca54e2fadd135e1437f3e3babaa1bbe3b33747f9",
            ),
            (
                "Barrick_Q3_2024_Mine_Stats.xlsx",
                [2760, 2210, 0, 177, 373, 0],
                "d23779d499353369690f833f38cb1d33f0f9a780325973db0ee1c517b012b90b",
            ),
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                [2760, 2213, 0, 173, 372, 2],
                "bd7b23ee73b2f2782cc20fb3462cf9407ed83f9d01aa748d4bab15e76d8cf31f",
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                [1292, 1095, 0, 52, 145, 0],
                "f41e13223eb52d3be07fc4181735156df4b9385467e9722dd4446095b57c0abc",
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                [2584, 2213, 0, 82, 288, 1],
                "3915d7b17e746664b0a0c10e92c6d3b0dc975395974f564dba40b74b596dda9b",
            ),
            (
                "Barrick_Q3_2025_Mine_Stats.xlsx",
                [2584, 2220, 0, 82, 281, 1],
                "c8a90061ceca3e84bb4dca7828bb0660bcc46faf4eb7ef2b53b7fa39b1a75720",
            ),
            (
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                [2616, 2266, 0, 44, 306, 0],
                "4705cfb746cc307058c17b1751fa9b3226758d75dc853d53e8fd3036b4a2e6ce",
            ),
            (
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                [1308, 1066, 3, 90, 152, 0],
                "2728f03979a853be77bc08c6796041496fef98af8d89077a8ca401fac0fe42f3",
            ),
            (
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                [2688, 2206, 4, 182, 298, 2],
                "92f18041a04855e5df35951a3511f1ae2db19509edb6cfdd763b4268b3210788",
            ),
        ];
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let sources = load_registered_sources(&profile, &root)
            .expect("registered files present and unaltered");
        let dump = std::env::var_os("FMINE_MINE_STATS_DUMP").map(PathBuf::from);
        // The registered vocabulary is in force: every label of the ten files must be registered.
        let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
        assert!(
            !vocabulary.is_empty(),
            "barrick registers a Mine Stats vocabulary"
        );
        let mut reads = BTreeMap::new();
        for src in sources.iter().filter(|s| s.kind == SourceKind::MineStats) {
            let read = read_mine_stats(&src.workbook, &src.quarter, Some(&vocabulary))
                .expect("registered quarter");
            let got = [
                read.values.len(),
                read.count("number"),
                read.numeric_zeros(),
                read.count("nil"),
                read.count("na"),
                read.count("missing"),
            ];
            let digest = read.digest().to_hex();
            eprintln!(
                "{{\"file\":\"{}\",\"sha256\":\"{}\",\"blocks\":{},\"cells\":{},\"cells_numeric\":{},\"numeric_zero\":{},\
                 \"cells_nil\":{},\"cells_na\":{},\"cells_missing\":{},\"refusals\":{},\"digest\":\"{digest}\"}}",
                src.file_name,
                Digest32(src.workbook.sha256).to_hex(),
                read.blocks,
                got[0],
                got[1],
                got[2],
                got[3],
                got[4],
                got[5],
                read.refusals.len()
            );
            if let Some(dir) = &dump {
                let text: String = read
                    .canonical_lines()
                    .iter()
                    .map(|l| format!("{l}\n"))
                    .collect();
                std::fs::write(dir.join(format!("{}.rust.tsv", src.file_name)), text)
                    .expect("write dump");
            }
            reads.insert(src.file_name.clone(), (read, got, digest));
        }
        assert_eq!(
            reads.len(),
            oracle.len(),
            "every registered Mine Stats file through 2026Q2 is read"
        );
        if let Some(dir) = std::env::var_os("FMINE_MINE_STATS_ORACLE").map(PathBuf::from) {
            let mut differing = 0;
            for (file, ..) in oracle {
                let short = file
                    .strip_prefix("Barrick_")
                    .and_then(|f| f.strip_suffix("_Mine_Stats.xlsx"))
                    .unwrap_or(file);
                let text = std::fs::read_to_string(dir.join(format!("{short}.oracle.tsv")))
                    .expect("the oracle's lines for every file");
                let theirs: Vec<String> = text.lines().map(str::to_string).collect();
                for line in mismatch_lines(file, &reads[*file].0.canonical_lines(), &theirs) {
                    eprintln!("{line}");
                    differing += 1;
                }
            }
            eprintln!("{{\"mismatches\":{differing}}}");
            assert_eq!(differing, 0, "cells where the reads differ (logged above)");
        }
        for (file, counts, digest) in oracle {
            let (read, got, d) = reads
                .get(*file)
                .unwrap_or_else(|| panic!("{file} not loaded"));
            assert!(read.refusals.is_empty(), "{file}: {:?}", read.refusals);
            assert!(
                read.hidden_sheets.is_empty(),
                "{file}: {:?}",
                read.hidden_sheets
            );
            assert_eq!(got, counts, "{file} counts");
            assert_eq!(d, digest, "{file} canonical-line digest");
        }

        // Plan §21.6 / C1: the Q1 and Q2 2026 files hold 3,996 parsed cells, and the seven numeric-zero
        // placeholders (recovery of divested Hemlo and Tongon, C1 DR-07) are numbers with stored text "0".
        let in_2026: usize = [
            "Barrick_Q1_2026_Mine_Stats.xlsx",
            "Barrick_Q2_2026_Mine_Stats.xlsx",
        ]
        .iter()
        .map(|f| reads[*f].0.values.len())
        .sum();
        assert_eq!(in_2026, 3_996);
        let mut zeros: Vec<&str> = reads
            .values()
            .flat_map(|(r, ..)| r.values.iter().filter(|v| v.is_numeric_zero()))
            .map(|v| {
                assert_eq!(
                    v.value,
                    MineStatsCell::Number {
                        raw_text: "0".into(),
                        value: 0.0
                    },
                    "{}",
                    v.locator
                );
                v.locator.as_str()
            })
            .collect();
        zeros.sort_unstable();
        assert_eq!(
            zeros,
            [
                "2026Q1:Minestats1!N27",
                "2026Q1:Minestats1!N28",
                "2026Q1:Minestats3!F19",
                "2026Q2:Minestats1!N27",
                "2026Q2:Minestats1!N28",
                "2026Q2:Minestats3!F19",
                "2026Q2:Minestats3!F64",
            ]
        );

        // W1 §4, the M0 target-cell census: 820 = 744 numeric, 33 '— ', 11 '—', 28 n/a, 4 numeric zero.
        // W1 states the counts, not the selection; the selection in `is_m0_target` is the one that reproduces
        // them from W1's own read (checked in Python on long_all3.json before this assertion was written).
        let mut census = BTreeMap::new();
        for (r, ..) in reads.values() {
            for v in r.values.iter().filter(|v| is_m0_target(v, &r.quarter[..4])) {
                let k = match &v.value {
                    MineStatsCell::Number { .. } if v.is_numeric_zero() => "zero",
                    MineStatsCell::Number { .. } => "numeric",
                    MineStatsCell::Nil { token } if token == "— " => "dash_space",
                    MineStatsCell::Nil { .. } => "dash",
                    MineStatsCell::NotApplicable { .. } => "na",
                    MineStatsCell::Missing => "missing",
                };
                *census.entry(k).or_insert(0usize) += 1;
            }
        }
        assert_eq!(
            census,
            BTreeMap::from([
                ("dash", 11),
                ("dash_space", 33),
                ("na", 28),
                ("numeric", 744),
                ("zero", 4)
            ])
        );

        // Mutation: two wrong readers, built from the reads (never from the oracle), fail the Q2 2026 known
        // answer. Without footnote stripping a row label keeps its footnote number; a reader fixed to Q2 2025's
        // rows reads Q2 2026 at the coordinates Q2 2025 used, a year later.
        let q2_2026 = "Barrick_Q2_2026_Mine_Stats.xlsx";
        let known = oracle
            .iter()
            .find(|(f, ..)| *f == q2_2026)
            .map(|(.., d)| *d)
            .expect("Q2 2026 known answer");
        let read = &reads[q2_2026].0;
        let footnoted = read.values.iter().filter(|v| v.footnote.is_some()).count();
        let unstripped: Vec<String> = read
            .values
            .iter()
            .map(|v| {
                let mut m = v.clone();
                if m.footnote.is_some() {
                    m.label.clone_from(&v.label_as_printed);
                }
                m.canonical_line()
            })
            .collect();
        let workbook = &sources
            .iter()
            .find(|s| s.file_name == q2_2026)
            .expect("Q2 2026 loaded")
            .workbook;
        let (mut fixed, mut refused) = (Vec::new(), 0usize);
        for v in &reads["Barrick_Q2_2025_Mine_Stats.xlsx"].0.values {
            let at = workbook
                .sheet(&v.sheet)
                .zip(fmine_io::xlsx::parse_a1_ref(&v.cell));
            match at.map(|(sheet, (row, col))| typed_cell(sheet, row, col)) {
                Some(Ok(value)) => {
                    let mut m = v.clone();
                    m.period = year_later(&v.period);
                    m.value = value;
                    fixed.push(m.canonical_line());
                }
                _ => refused += 1,
            }
        }
        let real: BTreeSet<String> = read.canonical_lines().into_iter().collect();
        let misread = fixed.iter().filter(|l| !real.contains(*l)).count();
        eprintln!("{{\"mutation\":\"no_footnote_stripping\",\"labels_changed\":{footnoted}}}");
        eprintln!(
            "{{\"mutation\":\"fixed_rows_of_2025Q2\",\"cells\":{},\"refused\":{refused},\
             \"not_in_the_read\":{misread}}}",
            fixed.len()
        );
        assert!(footnoted > 0, "Q2 2026 prints footnoted row labels");
        assert_ne!(lines_digest(unstripped), known, "unstripped labels fail");
        assert!(misread > 0, "the Q2 2026 rows moved since Q2 2025");
        assert_ne!(lines_digest(fixed), known, "fixed rows fail");
    }
}

#[cfg(test)]
mod mine_ledger {
    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    };
    use fmine_numerics::exact_feas::Rat;
    use fmine_public::key::{StatusKind, SubjectStatus};
    use fmine_public::mine_ledger::{
        Component, Family, Instance, LedgerFile, LedgerRules, MineLedger, Reason, State,
        mine_ledger, range_text,
    };
    use fmine_public::mine_stats::{MineStatsRead, read_mine_stats};
    use fmine_public::sources::{RegionMemberRow, RegisteredShare};
    use fmine_registry::sources::PrintedShareCorrection;

    /// A summary row: the label, then 100% production, attributable production and attributable sold for the
    /// release's own year (columns B–D). A cell is `""` for blank, `—` or `n/a` as printed, a number as its
    /// stored text under General, or `#` (`##`) and the stored text for a number shown under `0.0` (`0.00`).
    type Row<'a> = (&'a str, [&'a str; 3]);

    /// `(n, QTD rows, YTD rows)` for release `2030Qn`; a first quarter has no `YTD` sheet.
    type Release<'a> = (u8, Vec<Row<'a>>, Vec<Row<'a>>);

    const NONE: LedgerRules<'static> = LedgerRules {
        regions: &[],
        region_members: &[],
        statuses: &[],
        corrections: &[],
        aliases: &[],
        labels: &[],
    };

    const GOLD: [&str; 3] = [
        "Gold produced (000s oz 100%)",
        "Gold produced (000s oz attributable)",
        "Gold sold (000s oz attributable)",
    ];

    const COPPER: [&str; 3] = [
        "Copper produced (thousands of tonnes 100%)",
        "Copper produced (thousands of tonnes attributable)",
        "Copper sold (thousands of tonnes attributable)",
    ];

    fn text(s: &str) -> MinimalCell {
        MinimalCell::Text(s.to_string())
    }

    /// A data cell from its spec (see [`Row`]); a spec holding `—` is that text as printed. `None` for blank.
    fn data(v: &str) -> Option<MinimalCell> {
        let number = |raw: &str, xf| MinimalCell::Styled {
            raw: raw.to_string(),
            xf,
        };
        match v {
            "" => None,
            _ if v.contains('—') || v == "n/a" => Some(text(v)),
            _ => Some(match (v.strip_prefix("##"), v.strip_prefix('#')) {
                (Some(raw), _) => number(raw, 2),
                (None, Some(raw)) => number(raw, 1),
                (None, None) => number(v, 0),
            }),
        }
    }

    /// A summary sheet with one block of the release's own year: `headers` in B4–D4, rows from row 5.
    fn summary(name: &str, months: &str, headers: [&str; 3], rows: &[Row<'_>]) -> MinimalSheet {
        let rows: Vec<(&str, &[&str])> = rows.iter().map(|(l, v)| (*l, v.as_slice())).collect();
        summary_cols(name, months, &headers, &rows)
    }

    /// [`summary`] with up to six columns, B–G.
    fn summary_cols(
        name: &str,
        months: &str,
        headers: &[&str],
        rows: &[(&str, &[&str])],
    ) -> MinimalSheet {
        const COLS: [&str; 6] = ["B", "C", "D", "E", "F", "G"];
        let mut cells = vec![
            (
                "A3".to_string(),
                text(&format!("For the {months} months ended December 31,")),
            ),
            (
                "B3".to_string(),
                MinimalCell::Styled {
                    raw: "2030".to_string(),
                    xf: 0,
                },
            ),
        ];
        for (col, h) in COLS.into_iter().zip(headers) {
            cells.push((format!("{col}4"), text(h)));
        }
        for (k, (label, values)) in rows.iter().enumerate() {
            let row = 5 + k;
            cells.push((format!("A{row}"), text(label)));
            for (col, v) in COLS.into_iter().zip(values.iter()) {
                if let Some(c) = data(v) {
                    cells.push((format!("{col}{row}"), c));
                }
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    /// The ledger over `(quarter, sheets)` workbooks, with the reader's refusal locators.
    fn run(
        books: Vec<(String, Vec<MinimalSheet>)>,
        rules: &LedgerRules<'_>,
    ) -> (MineLedger, Vec<String>) {
        let styles = MinimalStyles {
            num_fmts: vec![(164, "0.0".to_string())],
            xfs: vec![0, 164, 2],
        };
        let books: Vec<(String, Workbook)> = books
            .into_iter()
            .map(|(q, sheets)| {
                let bytes = build_minimal_xlsx_styled(&sheets, Some(&styles));
                let wb = Workbook::from_bytes("synthetic_mine_ledger.xlsx", &bytes)
                    .expect("synthetic workbook parses");
                (q, wb)
            })
            .collect();
        let reads: Vec<MineStatsRead> = books
            .iter()
            .map(|(q, wb)| read_mine_stats(wb, q, None).expect("reads"))
            .collect();
        let refusals = reads
            .iter()
            .flat_map(|r| &r.refusals)
            .map(|r| r.locator.clone())
            .collect();
        let files: Vec<LedgerFile<'_>> = books
            .iter()
            .zip(&reads)
            .map(|((quarter, workbook), read)| LedgerFile {
                quarter,
                read,
                workbook,
            })
            .collect();
        (mine_ledger(&files, rules), refusals)
    }

    fn ledger(releases: &[Release<'_>], rules: &LedgerRules<'_>) -> MineLedger {
        let books = releases
            .iter()
            .map(|(n, qtd, ytd)| {
                let mut sheets = vec![summary("QTD", "three", GOLD, qtd)];
                if *n > 1 {
                    let months = ["three", "six", "nine", "twelve"][usize::from(*n) - 1];
                    sheets.push(summary("YTD", months, GOLD, ytd));
                }
                (format!("2030Q{n}"), sheets)
            })
            .collect();
        let (l, refusals) = run(books, rules);
        assert!(refusals.is_empty(), "{refusals:?}");
        l
    }

    /// Release `2030Q1`: a `QTD` sheet of `metal` headers and `rows`, beside the given `Minestats*` sheets.
    fn site_ledger(
        metal: [&str; 3],
        rows: &[Row<'_>],
        sheets: Vec<MinimalSheet>,
        rules: &LedgerRules<'_>,
    ) -> (MineLedger, Vec<String>) {
        let mut all = vec![summary("QTD", "three", metal, rows)];
        all.extend(sheets);
        run(vec![("2030Q1".to_string(), all)], rules)
    }

    fn at<'a>(l: &'a MineLedger, family: Family, anchor: &str) -> &'a Instance {
        l.instances
            .iter()
            .find(|i| i.family == family && i.anchor == anchor)
            .unwrap_or_else(|| panic!("no {} instance at {anchor}", family.as_str()))
    }

    fn sets(c: &Component) -> Vec<Vec<&str>> {
        c.corrections
            .iter()
            .map(|s| s.cells.iter().map(String::as_str).collect())
            .collect()
    }

    fn refused(reason: Reason, at: Option<&str>) -> State {
        State::Refused {
            reason,
            at: at.map(str::to_string),
        }
    }

    /// Carlin at 50%: Q1 100% production printed 300 where the other cells imply 200 (the shape of E113).
    fn e113_shape<'a>() -> Vec<Release<'a>> {
        vec![
            (1, vec![("Carlin (50%)", ["300", "100", "90"])], vec![]),
            (
                2,
                vec![("Carlin (50%)", ["150", "75", "70"])],
                vec![("Carlin (50%)", ["350", "175", "160"])],
            ),
        ]
    }

    fn loulo_withheld() -> [SubjectStatus; 1] {
        [SubjectStatus {
            subject: "Loulo-Gounkoto".to_string(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "E818".to_string(),
        }]
    }

    fn member(region: &str, member: &str, first: &str) -> RegionMemberRow {
        RegionMemberRow {
            region: region.to_string(),
            member: member.to_string(),
            first: first.to_string(),
            last: None,
        }
    }

    /// Locators on release 2030Q1's `QTD` sheet, `None` kept.
    fn locs(cells: &[Option<&str>]) -> Vec<Option<String>> {
        cells
            .iter()
            .map(|c| c.map(|c| format!("2030Q1:QTD!{c}")))
            .collect()
    }

    fn sum_state(l: &MineLedger, family: Family, cell: &str) -> State {
        at(l, family, &format!("2030Q1:QTD!{cell}")).state.clone()
    }

    /// The first region sum on the 100% column.
    fn region_b(l: &MineLedger) -> &Instance {
        l.instances
            .iter()
            .find(|i| i.family == Family::SitesToRegion && i.field == "au_prod_100_koz")
            .expect("a region sum on the 100% column")
    }

    /// Release 2030Q1, rows 5–10: North America = Carlin + Hemlo, Latin America = Pueblo Viejo, `Total Gold` = the
    /// two regions.
    fn americas<'a>(na: [&'a str; 3], total: [&'a str; 3]) -> Vec<Release<'a>> {
        vec![(
            1,
            vec![
                ("Carlin (50%)", ["100", "50", "45"]),
                ("Hemlo (100%)", ["50", "50", "45"]),
                ("North America", na),
                ("Pueblo Viejo (60%)", ["100", "60", "55"]),
                ("Latin America", ["100", "60", "55"]),
                ("Total Gold", total),
            ],
            vec![],
        )]
    }

    #[test]
    fn test_one_shared_cell_is_the_one_minimum_set() {
        let l = ledger(&e113_shape(), &NONE);
        let own = at(&l, Family::Ownership, "2030Q1:QTD!C5");
        let sum = at(&l, Family::QuarterSum, "2030Q2:YTD!B5");
        assert_eq!((own.state.code(), sum.state.code()), ("iii", "iii"));
        assert_eq!(own.share, Rat::new(1, 2).ok());
        assert_eq!(
            (
                l.count(Family::Ownership, "i"),
                l.count(Family::QuarterSum, "i")
            ),
            (2, 2)
        );
        assert_eq!(l.instances.len(), 6);
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert_eq!(own.component.as_deref(), Some(c.id.as_str()));
        assert_eq!(sum.component, own.component);
        assert!(c.minimum && !c.joint_only);
        assert_eq!(sets(c), [["2030Q1:QTD!B5"]]);
        // The range B5 can take from the other cells holds 200 and not the printed 300.
        let [(cell, Ok(range))] = c.corrections[0].implied.as_slice() else {
            panic!("one implied range: {:?}", c.corrections[0].implied);
        };
        assert_eq!(cell, "2030Q1:QTD!B5");
        assert_eq!(range.contains(Rat::from_integer(200)), Ok(true));
        assert_eq!(range.contains(Rat::from_integer(300)), Ok(false));
        // The sold column is its own component, feasible.
        assert_eq!(l.components, 2);
    }

    /// Every identity is consistent alone; together they are not. 51 attributable at 50% puts each quarter's
    /// true 100% production at 101 or more, so H1 is at least 202, above the 201.5 that 201 allows.
    #[test]
    fn test_joint_only_component_keeps_its_instances_consistent() {
        let carlin = |v| vec![("Carlin (50%)", v)];
        let l = ledger(
            &[
                (1, carlin(["101", "51", "40"]), vec![]),
                (2, carlin(["101", "51", "40"]), carlin(["201", "101", "80"])),
            ],
            &NONE,
        );
        assert!(l.instances.iter().all(|i| i.state == State::Consistent));
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert!(c.joint_only && c.minimum);
        assert_eq!(c.instances.len(), 5);
        assert_eq!(
            sets(c),
            [["2030Q1:QTD!C5"], ["2030Q2:QTD!C5"], ["2030Q2:YTD!B5"]]
        );
        let ranges: Vec<(&str, String)> = c
            .corrections
            .iter()
            .flat_map(|s| &s.implied)
            .map(|(cell, r)| (cell.as_str(), range_text(r)))
            .collect();
        assert_eq!(
            ranges,
            [
                ("2030Q1:QTD!C5", "[201/4, 201/4]".to_string()),
                ("2030Q2:QTD!C5", "[201/4, 201/4]".to_string()),
                ("2030Q2:YTD!B5", "[202, 203]".to_string()),
            ]
        );
    }

    #[test]
    fn test_unprinted_share_is_refused_and_aggregates_pose_no_ownership() {
        let regions = ["Nevada".to_string()];
        let rules = LedgerRules {
            regions: &regions,
            ..NONE
        };
        let rows = vec![
            ("Hemlo", ["100", "100", "90"]),
            ("Nevada", ["200", "100", "90"]),
            ("Total Gold", ["300", "200", "180"]),
        ];
        let l = ledger(&[(1, rows, vec![])], &rules);
        let owned: Vec<&Instance> = l
            .instances
            .iter()
            .filter(|i| i.family == Family::Ownership)
            .collect();
        let [hemlo] = owned.as_slice() else {
            panic!("ownership for Hemlo only: {:?}", l.instances);
        };
        // 100 = 100% × 100 would hold; an unprinted share is never read as 100%.
        assert_eq!(hemlo.subject, "Hemlo");
        assert_eq!(hemlo.state, refused(Reason::ShareUnprinted, None));
        assert_eq!((hemlo.share, hemlo.printed_share.as_deref()), (None, None));
    }

    #[test]
    fn test_registered_share_replaces_the_printed_one_kept_beside() {
        let rows = || vec![(1, vec![("Porgera (47.5%)", ["200", "49", "45"])], vec![])];
        let porgera = |printed: &str| PrintedShareCorrection {
            subject: "Porgera".to_string(),
            printed: printed.to_string(),
            num: 49,
            den: 200,
            evidence: "W1-DR-01".to_string(),
        };
        let state = |corrections: &[PrintedShareCorrection]| {
            let l = ledger(
                &rows(),
                &LedgerRules {
                    corrections,
                    ..NONE
                },
            );
            at(&l, Family::Ownership, "2030Q1:QTD!C5").clone()
        };
        // 47.5% of 200 is 95, not 49.
        assert_eq!(state(&[]).state, State::Discrepancy);
        // A correction for another printed share does not apply.
        assert_eq!(state(&[porgera("50")]).state, State::Discrepancy);
        let corrected = state(&[porgera("47.5")]);
        assert_eq!(corrected.state, State::Consistent);
        assert_eq!(corrected.share, Rat::new(49, 200).ok());
        assert_eq!(corrected.printed_share.as_deref(), Some("47.5"));
    }

    #[test]
    fn test_withheld_subject_is_excluded_for_the_periods_it_covers() {
        let lg = |v| vec![("Loulo-Gounkoto (80%)", v)];
        let releases = vec![
            (1, lg(["100", "10", "9"]), vec![]),
            (2, lg(["100", "80", "70"]), lg(["200", "90", "79"])),
        ];
        let statuses = loulo_withheld();
        let withheld = refused(
            Reason::Withheld {
                evidence: "E818".to_string(),
            },
            None,
        );
        let l = ledger(
            &releases,
            &LedgerRules {
                statuses: &statuses,
                ..NONE
            },
        );
        assert_eq!(at(&l, Family::Ownership, "2030Q1:QTD!C5").state, withheld);
        assert_eq!(
            at(&l, Family::Ownership, "2030Q2:QTD!C5").state,
            State::Consistent
        );
        // H1 is not entirely within Q1: decided, and 80% of 200 is not 90.
        assert_eq!(
            at(&l, Family::Ownership, "2030Q2:YTD!C5").state,
            State::Discrepancy
        );
        for col in ["B", "C", "D"] {
            let anchor = format!("2030Q2:YTD!{col}5");
            assert_eq!(at(&l, Family::QuarterSum, &anchor).state, withheld);
        }
        // Without the status the Q1 row is decided.
        let open = ledger(&releases, &NONE);
        assert_eq!(
            at(&open, Family::Ownership, "2030Q1:QTD!C5").state,
            State::Discrepancy
        );
    }

    #[test]
    fn test_region_whose_members_change_between_releases_is_excluded() {
        let africa = |v| vec![("Africa", v)];
        let releases = vec![
            (1, africa(["100", "60", "50"]), vec![]),
            (
                2,
                africa(["100", "60", "50"]),
                africa(["250", "120", "100"]),
            ),
        ];
        let regions = ["Africa".to_string()];
        let member = |m: &str, first: &str| RegionMemberRow {
            region: "Africa".to_string(),
            member: m.to_string(),
            first: first.to_string(),
            last: None,
        };
        let states = |members: &[RegionMemberRow]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: members,
                ..NONE
            };
            let l = ledger(&releases, &rules);
            assert_eq!(
                l.count(Family::Ownership, "i") + l.count(Family::Ownership, "iii"),
                0
            );
            ["B", "C", "D"].map(|col| {
                let anchor = format!("2030Q2:YTD!{col}5");
                at(&l, Family::QuarterSum, &anchor).state.clone()
            })
        };
        // One map: decided (250 is not 100 + 100).
        assert_eq!(
            states(&[member("Kibali", "2030Q1")]),
            [State::Discrepancy, State::Consistent, State::Consistent]
        );
        // Tongon joins in Q2: Q1 + Q2 would add two different regions.
        let cross = refused(Reason::CrossMapRegion, None);
        assert_eq!(
            states(&[member("Kibali", "2030Q1"), member("Tongon", "2030Q2")]),
            [cross.clone(), cross.clone(), cross]
        );
    }

    #[test]
    fn test_placeholders_blanks_and_repeated_rows() {
        let rows = vec![
            ("Tongon (89.7%)", ["—", "—", "—"]),
            ("Kibali (45%)", ["—", "45", "40"]),
            ("Hemlo (100%)", ["", "45", "40"]),
            ("Zaldivar (50%)", ["n/a", "n/a", ""]),
            ("Carlin (50%)", ["100", "50", "40"]),
            ("Carlin (50%)", ["100", "50", "40"]),
        ];
        let l = ledger(&[(1, rows, vec![])], &NONE);
        let state = |row: u32| {
            at(&l, Family::Ownership, &format!("2030Q1:QTD!C{row}"))
                .state
                .clone()
        };
        assert_eq!(state(5), State::NotPublished);
        assert_eq!(
            state(6),
            refused(Reason::PlaceholderOperand, Some("2030Q1:QTD!B6"))
        );
        assert_eq!(
            state(7),
            refused(Reason::BlankOperand, Some("2030Q1:QTD!B7"))
        );
        assert_eq!(state(8), State::NotPublished);
        // One subject on two rows of a block keeps neither value.
        assert_eq!(
            state(9),
            refused(Reason::OperandUnreadable, Some("2030Q1:QTD!B9"))
        );
        assert_eq!(l.instances.len(), 5);
    }

    /// The same stored text decides differently under different number formats: the box is the displayed
    /// value's, never the stored text's.
    #[test]
    fn test_displayed_value_sets_the_box() {
        let state = |b: &str, c: &str| {
            let l = ledger(&[(1, vec![("Carlin (50%)", [b, c, "40"])], vec![])], &NONE);
            at(&l, Family::Ownership, "2030Q1:QTD!C5").state.clone()
        };
        // Shown as 100.1 and 50.0: half of [100.05, 100.15] meets [49.95, 50.05].
        assert_eq!(state("#100.14", "#50.04"), State::Consistent);
        // Shown as stored, under 0.00: half of [100.135, 100.145] misses [50.035, 50.045].
        assert_eq!(state("##100.14", "##50.04"), State::Discrepancy);
        // General shows a non-integer by column width, which the file does not fix: refused, never guessed.
        assert_eq!(
            state("100.14", "#50.04"),
            refused(Reason::OperandUnreadable, Some("2030Q1:QTD!B5"))
        );
    }

    #[test]
    fn test_order_of_rows_and_files_changes_no_state() {
        let mut releases = e113_shape();
        for (_, qtd, ytd) in &mut releases {
            qtd.push(("Hemlo (100%)", ["20", "20", "18"]));
            ytd.push(("Hemlo (100%)", ["42", "40", "36"]));
        }
        let project = |l: &MineLedger| {
            let mut v: Vec<_> = l
                .instances
                .iter()
                .map(|i| {
                    (
                        i.family,
                        i.subject.clone(),
                        i.field.clone(),
                        i.period.code(),
                        i.state.code(),
                        i.component.is_some(),
                    )
                })
                .collect();
            v.sort();
            (v, l.components, l.infeasible.len())
        };
        let base = ledger(&releases, &NONE);
        let mut permuted = releases.clone();
        permuted.reverse();
        for (_, qtd, ytd) in &mut permuted {
            qtd.reverse();
            ytd.reverse();
        }
        assert_eq!(project(&ledger(&permuted, &NONE)), project(&base));
        // Hemlo's H1 100% (42 against 20 + 20, and against its own 40 attributable) is a second component.
        assert_eq!(base.infeasible.len(), 2);
    }

    #[test]
    fn test_mutations_move_the_state() {
        let own_q2 = |releases: &[Release<'_>]| {
            let l = ledger(releases, &NONE);
            at(&l, Family::Ownership, "2030Q2:QTD!C5").state.clone()
        };
        let base = e113_shape();
        assert_eq!(own_q2(&base), State::Consistent);
        // A sign flip on the attributable cell.
        let mut flipped = base.clone();
        flipped[1].1[0].1[1] = "-75";
        assert_eq!(own_q2(&flipped), State::Discrepancy);
        // The share dropped from the label.
        let mut unshared = base.clone();
        unshared[1].1[0].0 = "Carlin";
        assert_eq!(own_q2(&unshared), refused(Reason::ShareUnprinted, None));
        // The first quarter's file missing: the H1 sums are refused, the Q2 rows still decided.
        let l = ledger(&base[1..], &NONE);
        assert_eq!(
            at(&l, Family::QuarterSum, "2030Q2:YTD!B5").state,
            refused(Reason::ReleaseMissing, None)
        );
        assert_eq!(
            at(&l, Family::Ownership, "2030Q2:QTD!C5").state,
            State::Consistent
        );
    }

    #[test]
    fn test_region_and_total_sums_hold_within_the_rounding_slack() {
        let regions = ["North America".to_string(), "Latin America".to_string()];
        let members = [
            member("North America", "Carlin", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
            member("Latin America", "Pueblo Viejo", "2030Q1"),
        ];
        let rules = LedgerRules {
            regions: &regions,
            region_members: &members,
            ..NONE
        };
        // Three operands shown to the unit: 100 + 50 meets the printed region within 1.5.
        for (printed, code) in [
            ("148", "iii"),
            ("149", "i"),
            ("150", "i"),
            ("151", "i"),
            ("152", "iii"),
        ] {
            let l = ledger(
                &americas([printed, "100", "90"], ["250", "160", "145"]),
                &rules,
            );
            let state = sum_state(&l, Family::SitesToRegion, "B7");
            assert_eq!(state.code(), code, "North America printed {printed}");
        }
        let l = ledger(
            &americas(["150", "102", "90"], ["250", "162", "145"]),
            &rules,
        );
        let na = at(&l, Family::SitesToRegion, "2030Q1:QTD!B7");
        assert_eq!(
            (na.subject.as_str(), na.field.as_str()),
            ("North America", "au_prod_100_koz")
        );
        assert_eq!(na.cells, locs(&[Some("B5"), Some("B6"), Some("B7")]));
        assert_eq!(
            sum_state(&l, Family::SitesToRegion, "C7"),
            State::Discrepancy
        );
        // The total adds the regions as printed: North America's discrepancy does not reach it.
        let total = at(&l, Family::RegionToTotal, "2030Q1:QTD!C10");
        assert_eq!(
            (total.subject.as_str(), &total.state),
            ("Total Gold", &State::Consistent)
        );
        assert_eq!(total.cells, locs(&[Some("C7"), Some("C9"), Some("C10")]));
        assert_eq!(
            [Family::SitesToRegion, Family::RegionToTotal]
                .map(|f| [l.count(f, "i"), l.count(f, "iii")]),
            [[5, 1], [3, 0]]
        );
        // Decided alone and never posed: the discrepancy is in no component.
        assert!(l.infeasible.is_empty());
        assert!(
            l.instances
                .iter()
                .all(|i| i.component.is_none() && !i.nil_token)
        );
    }

    #[test]
    fn test_sum_operands_nil_token_and_placeholders() {
        let regions = ["North America".to_string()];
        let members = [
            member("North America", "Carlin", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
        ];
        let rules = LedgerRules {
            regions: &regions,
            region_members: &members,
            ..NONE
        };
        let states = |hemlo: [&str; 3], na: [&str; 3]| {
            let rows = vec![
                ("Carlin (50%)", ["100", "—", "45"]),
                ("Hemlo (100%)", hemlo),
                ("North America", na),
            ];
            let l = ledger(&[(1, rows, vec![])], &rules);
            ["B7", "C7", "D7"].map(|c| {
                let i = at(&l, Family::SitesToRegion, &format!("2030Q1:QTD!{c}"));
                (i.state.clone(), i.nil_token)
            })
        };
        let at6 = |c: &str| Some(format!("2030Q1:QTD!{c}6"));
        // The nil token is exactly 0 with no box: 100 + 0 meets 101 within 1.
        assert_eq!(
            states(["—", "—", "n/a"], ["101", "—", "45"]),
            [
                (State::Consistent, true),
                (State::NotPublished, false),
                (
                    State::Refused {
                        reason: Reason::PlaceholderOperand,
                        at: at6("D")
                    },
                    false
                ),
            ]
        );
        // 102 is past it; a blank comes before the placeholders; a nil region is 0 against 45.
        assert_eq!(
            states(["—", "", "—"], ["102", "50", "—"]),
            [
                (State::Discrepancy, true),
                (
                    State::Refused {
                        reason: Reason::BlankOperand,
                        at: at6("C")
                    },
                    false
                ),
                (State::Discrepancy, true),
            ]
        );
    }

    #[test]
    fn test_member_is_found_by_its_registered_label_and_refused_under_both() {
        let regions = ["North America".to_string()];
        let members = [
            member("North America", "Nevada Gold Mines", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
        ];
        let labels = [RegisteredShare {
            subject: "Nevada Gold Mines".to_string(),
            num: 1,
            den: 2,
            evidence: "E1".to_string(),
            label: "Nevada Gold Mines LLC".to_string(),
        }];
        let llc = ("Nevada Gold Mines LLC (50%)", ["100", "50", "45"]);
        let plain = ("Nevada Gold Mines (50%)", ["100", "50", "45"]);
        let hemlo = ("Hemlo (100%)", ["50", "50", "45"]);
        let na = ("North America", ["150", "100", "90"]);
        let sum = |rows: Vec<Row<'_>>, labels: &[RegisteredShare]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: &members,
                labels,
                ..NONE
            };
            let i = region_b(&ledger(&[(1, rows, vec![])], &rules)).clone();
            (i.state, i.cells)
        };
        assert_eq!(
            sum(vec![llc, hemlo, na], &labels),
            (
                State::Consistent,
                locs(&[Some("B5"), Some("B6"), Some("B7")])
            )
        );
        assert_eq!(
            sum(vec![llc, hemlo, na], &[]),
            (
                refused(Reason::OperandRowMissing, None),
                locs(&[None, Some("B6"), Some("B7")])
            )
        );
        // Printed under both names: refused at the row printed first, whichever name it carries.
        for rows in [vec![llc, plain, hemlo, na], vec![plain, llc, hemlo, na]] {
            assert_eq!(
                sum(rows, &labels),
                (
                    refused(Reason::OperandUnreadable, Some("2030Q1:QTD!B5")),
                    locs(&[Some("B5"), Some("B7"), Some("B8")])
                )
            );
        }
    }

    #[test]
    fn test_region_outside_the_map_and_missing_operand_rows_are_refused() {
        let regions = [
            "North America".to_string(),
            "Africa".to_string(),
            "Latin America".to_string(),
        ];
        let members = [
            member("North America", "Carlin", "2030Q1"),
            member("North America", "Cortez", "2030Q1"),
            member("Africa", "Kibali", "2031Q1"),
        ];
        let with_latin = [
            members.as_slice(),
            &[member("Latin America", "Pueblo Viejo", "2030Q1")],
        ]
        .concat();
        let rows = || {
            vec![
                ("Carlin (50%)", ["100", "50", "45"]),
                ("North America", ["100", "50", "45"]),
                ("Africa", ["10", "5", "4"]),
                ("Total Gold", ["100", "50", "45"]),
            ]
        };
        let with = |members: &[RegionMemberRow]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: members,
                ..NONE
            };
            ledger(&[(1, rows(), vec![])], &rules)
        };
        let l = with(&members);
        // Africa's row is printed, but its members join the map in 2031.
        assert_eq!(
            sum_state(&l, Family::SitesToRegion, "B7"),
            refused(Reason::RegionNotInMap, None)
        );
        // Cortez prints no row.
        let na = at(&l, Family::SitesToRegion, "2030Q1:QTD!B6");
        assert_eq!(na.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(na.cells, locs(&[Some("B5"), None, Some("B6")]));
        // The total adds the map's regions, so North America alone and not the printed Africa.
        let total = at(&l, Family::RegionToTotal, "2030Q1:QTD!B8");
        assert_eq!(total.state, State::Consistent);
        assert_eq!(total.cells, locs(&[Some("B6"), Some("B8")]));
        // A region of the map with no row refuses the total.
        let l = with(&with_latin);
        let total = at(&l, Family::RegionToTotal, "2030Q1:QTD!B8");
        assert_eq!(total.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(total.cells, locs(&[Some("B6"), None, Some("B8")]));
    }

    #[test]
    fn test_withheld_member_excludes_its_region_for_the_periods_it_covers() {
        let regions = ["Africa".to_string()];
        let members = [
            member("Africa", "Loulo-Gounkoto", "2030Q1"),
            member("Africa", "Kibali", "2030Q1"),
        ];
        let statuses = loulo_withheld();
        let quarter = || {
            vec![
                ("Loulo-Gounkoto (80%)", ["100", "80", "70"]),
                ("Kibali (45%)", ["100", "45", "40"]),
                ("Africa", ["200", "125", "110"]),
            ]
        };
        let h1 = vec![
            ("Loulo-Gounkoto (80%)", ["200", "160", "140"]),
            ("Kibali (45%)", ["200", "90", "80"]),
            ("Africa", ["400", "250", "220"]),
        ];
        let releases = vec![(1, quarter(), vec![]), (2, quarter(), h1)];
        let sums = |statuses: &[SubjectStatus]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: &members,
                statuses,
                ..NONE
            };
            let l = ledger(&releases, &rules);
            ["2030Q1:QTD!B7", "2030Q2:QTD!B7", "2030Q2:YTD!B7"]
                .map(|a| at(&l, Family::SitesToRegion, a).state.clone())
        };
        let withheld = refused(
            Reason::Withheld {
                evidence: "E818".to_string(),
            },
            None,
        );
        assert_eq!(
            sums(&statuses),
            [withheld, State::Consistent, State::Consistent]
        );
        assert_eq!(sums(&[]), [(); 3].map(|()| State::Consistent));
    }

    #[test]
    fn test_total_copper_sums_the_copper_sites_in_row_order() {
        let total = |d: &str| {
            let (l, refusals) = site_ledger(
                COPPER,
                &[
                    ("Zaldivar (50%)", ["50", "25", "24"]),
                    ("Lumwana (100%)", ["30", "30", "28"]),
                    ("Jabal Sayid (50%)", ["10", "5", "5"]),
                    ("Total Copper", ["90", "60", d]),
                ],
                vec![],
                &NONE,
            );
            assert!(refusals.is_empty(), "{refusals:?}");
            // 90 = 50 + 30 + 10 and 60 = 25 + 30 + 5; no copper row is a region.
            assert_eq!(
                [
                    sum_state(&l, Family::RegionToTotal, "B8"),
                    sum_state(&l, Family::RegionToTotal, "C8")
                ],
                [State::Consistent, State::Consistent]
            );
            assert!(
                l.instances
                    .iter()
                    .all(|i| i.family != Family::SitesToRegion)
            );
            at(&l, Family::RegionToTotal, "2030Q1:QTD!D8").clone()
        };
        let i = total("57");
        assert_eq!(
            (i.subject.as_str(), i.field.as_str()),
            ("Total Copper", "cu_sold_attr_kt")
        );
        assert_eq!(
            i.cells,
            locs(&[Some("D5"), Some("D6"), Some("D7"), Some("D8")])
        );
        // Four operands shown to the unit: 24 + 28 + 5 = 57 meets the printed total within 2, the bound included.
        for (printed, code) in [("54", "iii"), ("55", "i"), ("59", "i"), ("60", "iii")] {
            assert_eq!(total(printed).state.code(), code, "Total Copper {printed}");
        }
    }
}

#[cfg(test)]
mod mine_stats {
    use std::collections::{BTreeMap, BTreeSet};

    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx, col_index_to_letters,
    };
    use fmine_public::mine_stats::{
        LabelVocabulary, MineStatsCell, MineStatsRead, MineStatsValue, View, is_m0_target,
        read_mine_stats,
    };

    type Cells = Vec<(String, MinimalCell)>;

    fn t(s: &str) -> MinimalCell {
        MinimalCell::Text(s.to_string())
    }

    fn put(cells: &mut Cells, col: u32, row: u32, v: MinimalCell) {
        let at = format!("{}{row}", col_index_to_letters(col));
        cells.retain(|(r, _)| *r != at);
        cells.push((at, v));
    }

    /// A Minestats sheet and a QTD sheet in the issuer's layout. With `drift`, the same figures move the way
    /// the issuer's files drift between vintages (W1 §3): a site inserted in front (columns move right by two),
    /// a Revenue row inserted (rows move down by one), every footnote renumbered, a misspelled month and a
    /// doubled space in the block headers, and a 100%-basis column inserted in the summary.
    fn fixture(drift: bool) -> (Cells, Cells) {
        let (sh, rs) = if drift { (2, 1) } else { (0, 0) };
        let fnum = |base: u8| if drift { base + 1 } else { base };
        let mut ms = Cells::new();
        put(&mut ms, 1, 1, t("Mine Statistics (Gold)"));
        let mut sites = vec![
            (2 + sh, "Carlin (61.5%)".to_string()),
            (4 + sh, format!("Hemlo{}", fnum(3))),
        ];
        if drift {
            sites.push((2, "Pueblo Viejo (60%)".to_string()));
        }
        for header_row in [3, 12 + rs] {
            for (c, name) in &sites {
                put(&mut ms, *c, header_row, t(name));
            }
        }
        let three = if drift {
            "For the  three months ended June 30,"
        } else {
            "For the three months ended June 30,"
        };
        let six = if drift {
            "For the six months ended Juen 30,"
        } else {
            "For the six months ended June 30,"
        };
        for (row, label) in [(4, three), (13 + rs, six)] {
            put(&mut ms, 1, row, t(label));
            for (c, _) in &sites {
                put(&mut ms, *c, row, MinimalCell::Number(2030.0));
                put(&mut ms, c + 1, row, MinimalCell::Number(2029.0));
            }
        }
        if drift {
            put(&mut ms, 1, 5, t("Revenue ($ millions)"));
            for c in 2..=7 {
                put(&mut ms, c, 5, MinimalCell::Number(100.0));
            }
        }
        let n = MinimalCell::Number;
        let (carlin, hemlo) = (2 + sh, 4 + sh);
        let rows: Vec<(u32, String, [MinimalCell; 4], [MinimalCell; 2])> = vec![
            (
                5,
                "Tonnes processed (thousands)".into(),
                [n(1250.0), n(1190.0), t("— "), t("—")],
                [n(900.0), n(880.0)],
            ),
            (
                6,
                format!("Recovery rate (percent){}", fnum(2)),
                [n(0.91), n(0.9), n(0.0), t("n/a")],
                [n(0.85), n(0.84)],
            ),
            (
                8,
                "Processed".into(),
                [n(3.12), n(2.98), t(""), t("n/a")],
                [n(2.5), n(2.4)],
            ),
            (
                9,
                format!("Total cash costs ($/oz){}", fnum(4)),
                [n(1100.0), n(1050.0), t("n/a"), n(1400.0)],
                [n(950.0), n(900.0)],
            ),
            (
                14,
                "Tonnes processed (thousands)".into(),
                [n(2500.0), n(2380.0), n(300.0), n(310.0)],
                [n(1800.0), n(1700.0)],
            ),
        ];
        put(&mut ms, 1, 7 + rs, t("Average grade (grams per tonne)"));
        put(
            &mut ms,
            1,
            10 + rs,
            t("1 Barrick's share of the site's figures."),
        );
        for (row, label, [a, b, c, d], [pa, pb]) in rows {
            let row = row + rs;
            put(&mut ms, 1, row, t(&label));
            for (col, v) in [(carlin, a), (carlin + 1, b), (hemlo, c), (hemlo + 1, d)] {
                if v != t("") {
                    put(&mut ms, col, row, v);
                }
            }
            if drift {
                put(&mut ms, 2, row, pa);
                put(&mut ms, 3, row, pb);
            }
        }

        let mut qtd = Cells::new();
        put(&mut qtd, 1, 3, t("For the three months ended June 30,"));
        put(&mut qtd, 2, 3, n(2030.0));
        put(&mut qtd, 9, 3, n(2029.0));
        let qs = u32::from(drift);
        let headers = [
            "Gold produced (000s oz attributable)1".to_string(),
            "Cost of sales ($/oz)".to_string(),
            format!("All-in sustaining costs ($/oz){}", fnum(2)),
        ];
        for base in [2, 9] {
            if drift {
                put(&mut qtd, base, 4, t("Gold produced (000s oz 100%)"));
                put(&mut qtd, base, 5, n(667.0));
            }
            for (k, h) in headers.iter().enumerate() {
                put(&mut qtd, base + qs + k as u32, 4, t(h));
            }
        }
        let carlin_q = [
            n(410.0),
            n(1300.0),
            n(1650.0),
            n(400.0),
            n(1280.0),
            n(1600.0),
        ];
        let hemlo_q = [t("— "), t("— "), t("—"), n(50.0), n(1500.0), n(1900.0)];
        for (row, site, vals) in [
            (5, "Carlin (61.5%)".to_string(), carlin_q),
            (6, format!("Hemlo{}", fnum(2)), hemlo_q),
        ] {
            put(&mut qtd, 1, row, t(&site));
            for (k, v) in vals.into_iter().enumerate() {
                let k = k as u32;
                let col = if k < 3 { 2 + qs + k } else { 9 + qs + (k - 3) };
                put(&mut qtd, col, row, v);
            }
        }
        if drift {
            put(&mut qtd, 1, 7, t("Pueblo Viejo (60%)"));
            put(&mut qtd, 3, 7, n(120.0));
        }
        (ms, qtd)
    }

    fn workbook((ms, qtd): (Cells, Cells)) -> Workbook {
        let bytes = build_minimal_xlsx(&[
            MinimalSheet {
                name: "QTD".to_string(),
                hidden: false,
                cells: qtd,
            },
            MinimalSheet {
                name: "Minestats1".to_string(),
                hidden: false,
                cells: ms,
            },
        ]);
        Workbook::from_bytes("synthetic_mine_stats.xlsx", &bytes)
            .expect("synthetic workbook parses")
    }

    fn value<'a>(read: &'a MineStatsRead, locator: &str) -> &'a MineStatsValue {
        read.values
            .iter()
            .find(|v| v.locator == locator)
            .unwrap_or_else(|| panic!("no value at {locator}"))
    }

    /// Known answers on the synthetic layout: locators, periods, sites, sections, footnotes, raw tokens,
    /// numeric zero kept as a number, a blank in a row with data as Missing, and the summary field ids.
    #[test]
    fn test_synthetic_known_answers() {
        let read = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        assert_eq!(read.blocks, 3);
        assert_eq!(read.values.len(), 32);
        let kinds = ["number", "nil", "na", "missing"].map(|k| read.count(k));
        assert_eq!(kinds, [23, 5, 3, 1]);
        assert_eq!(read.numeric_zeros(), 1);

        let rec = value(&read, "2030Q2:Minestats1!B6");
        assert_eq!(
            (rec.label.as_str(), rec.footnote, rec.section.as_deref()),
            ("Recovery rate (percent)", Some(2), None)
        );
        assert_eq!(
            (
                rec.site.as_str(),
                rec.printed_share.as_deref(),
                rec.period.as_str()
            ),
            ("Carlin", Some("61.5"), "2030Q2")
        );
        assert_eq!(
            rec.value,
            MineStatsCell::Number {
                raw_text: "0.91".into(),
                value: 0.91
            }
        );
        let zero = value(&read, "2030Q2:Minestats1!D6");
        assert_eq!(
            (zero.site.as_str(), zero.printed_share.as_deref()),
            ("Hemlo", None)
        );
        assert_eq!(
            zero.value,
            MineStatsCell::Number {
                raw_text: "0".into(),
                value: 0.0
            }
        );
        assert_eq!(
            value(&read, "2030Q2:Minestats1!D5").value,
            MineStatsCell::Nil {
                token: "— ".into()
            }
        );
        assert_eq!(
            value(&read, "2030Q2:Minestats1!E5").value,
            MineStatsCell::Nil {
                token: "—".into()
            }
        );
        assert_eq!(
            value(&read, "2030Q2:Minestats1!E6").value,
            MineStatsCell::NotApplicable {
                token: "n/a".into()
            }
        );
        let processed = value(&read, "2030Q2:Minestats1!D8");
        assert_eq!(processed.value, MineStatsCell::Missing);
        assert_eq!(
            processed.section.as_deref(),
            Some("Average grade (grams per tonne)")
        );
        assert_eq!(value(&read, "2030Q2:Minestats1!C8").period, "2029Q2");
        assert!(
            read.values.iter().all(|v| v.cell != "B7"),
            "a label row with no data is not emitted"
        );
        let cum = value(&read, "2030Q2:Minestats1!C14");
        assert_eq!((cum.months, cum.period.as_str()), (6, "2029H1"));

        let cos = value(&read, "2030Q2:QTD!C5");
        assert_eq!(
            (cos.view, cos.label.as_str(), cos.period.as_str()),
            (View::Summary, "cos_usd_per_oz", "2030Q2")
        );
        assert_eq!(cos.label_as_printed, "Cost of sales ($/oz)");
        assert_eq!(value(&read, "2030Q2:QTD!K6").label, "aisc_usd_per_oz");
        assert_eq!(value(&read, "2030Q2:QTD!K6").period, "2029Q2");
        assert_eq!(
            value(&read, "2030Q2:QTD!B6").value,
            MineStatsCell::Nil {
                token: "— ".into()
            }
        );
        let m0 = read
            .values
            .iter()
            .filter(|v| is_m0_target(v, "2030"))
            .count();
        assert_eq!(
            m0,
            2 + 2 + 2 * 2,
            "QTD cost of sales and AISC for two sites, grade and recovery for two sites"
        );
        assert_eq!(
            read.digest(),
            read.clone().digest(),
            "the digest depends only on the values"
        );
    }

    /// The drifted layout reads to the same values: no coordinate from one vintage is used on another.
    #[test]
    fn test_drifted_layout_reads_identically() {
        let key = |v: &MineStatsValue| {
            (
                v.view.as_str(),
                v.period.clone(),
                v.site.clone(),
                v.label.clone(),
                v.section.clone(),
            )
        };
        let base = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        let drift = read_mine_stats(&workbook(fixture(true)), "2030Q2", None).expect("read");
        assert!(drift.refusals.is_empty(), "{:?}", drift.refusals);
        assert_eq!(drift.blocks, 3);
        let base_map: BTreeMap<_, _> = base
            .values
            .iter()
            .map(|v| (key(v), v.value.clone()))
            .collect();
        let drift_map: BTreeMap<_, _> = drift
            .values
            .iter()
            .map(|v| (key(v), v.value.clone()))
            .collect();
        assert_eq!(base_map.len(), base.values.len(), "keys are unique");
        for (k, v) in &base_map {
            assert_eq!(drift_map.get(k), Some(v), "{k:?}");
        }
        let extra: BTreeSet<_> = drift_map
            .keys()
            .filter(|k| !base_map.contains_key(*k))
            .map(|k| (k.2.clone(), k.3.clone()))
            .collect();
        let extra_sites_or_labels: BTreeSet<_> = extra
            .iter()
            .map(|(s, l)| {
                if s == "Pueblo Viejo" {
                    s.as_str()
                } else {
                    l.as_str()
                }
            })
            .collect();
        assert_eq!(
            extra_sites_or_labels,
            BTreeSet::from(["Pueblo Viejo", "Revenue ($ millions)", "au_prod_100_koz"]),
            "only the inserted site, row and column are new"
        );
        assert_ne!(
            base.digest(),
            drift.digest(),
            "the locators moved, so the canonical lines differ"
        );
    }

    /// A block is found by its period word, not its month: W1's `Q4_2025:Minestats1!A64` prints "Decemer".
    #[test]
    fn test_block_header_month_typo_reads() {
        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 13, t("For the Twelve months ended Decemer 31,"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q4", None).expect("read");
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        let year: BTreeSet<&str> = read
            .values
            .iter()
            .filter(|v| v.view == View::Minestats && v.months == 12)
            .map(|v| v.period.as_str())
            .collect();
        assert_eq!(year, BTreeSet::from(["2029FY", "2030FY"]));
        assert_eq!(read.blocks, 3, "the fixture's three blocks, none lost");
    }

    /// Negative controls: each refusal names its cell, and nothing is emitted for it.
    #[test]
    fn test_refusals_name_the_cell() {
        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 4, 5, t("x"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!D5");
        assert!(
            read.refusals[0].reason.contains("outside the set"),
            "{}",
            read.refusals[0].reason
        );
        assert_eq!(read.values.len(), 31);

        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 9, t("Tonnes processed (thousands)2"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!A9");
        assert!(read.refusals[0].reason.contains("appears twice"));
        assert_eq!(read.values.len(), 28, "the repeated row is not emitted");

        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 13, t("For the fortnight ended June 30,"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!A13");
        assert_eq!(
            (read.blocks, read.values.len()),
            (2, 28),
            "the unknown block is not read"
        );

        let (ms, mut qtd) = fixture(false);
        put(&mut qtd, 3, 5, MinimalCell::Error("#REF!".into()));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:QTD!C5");

        let refused =
            read_mine_stats(&workbook(fixture(false)), "2030-Q2", None).expect_err("bad quarter");
        assert!(refused.to_string().contains("not YYYYQn"), "{refused}");
    }

    /// Under the vocabulary of the base layout, the drifted layout reads the same except for the one row whose
    /// label is new (Revenue), which is refused by locator, in both blocks it would appear in.
    #[test]
    fn test_unregistered_label_is_refused_by_locator() {
        let base = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        let vocabulary: LabelVocabulary = base
            .values
            .iter()
            .filter(|v| v.view == View::Minestats)
            .map(|v| (v.section.clone(), v.label.clone()))
            .collect();
        let again =
            read_mine_stats(&workbook(fixture(false)), "2030Q2", Some(&vocabulary)).expect("read");
        assert!(again.refusals.is_empty() && again.values == base.values);

        let drift =
            read_mine_stats(&workbook(fixture(true)), "2030Q2", Some(&vocabulary)).expect("read");
        assert_eq!(drift.refusals.len(), 1, "{:?}", drift.refusals);
        assert_eq!(drift.refusals[0].locator, "2030Q2:Minestats1!A5");
        assert!(
            drift.refusals[0]
                .reason
                .contains("\"Revenue ($ millions)\"")
        );
        assert!(
            drift
                .values
                .iter()
                .all(|v| v.label != "Revenue ($ millions)")
        );

        let mut narrow = vocabulary.clone();
        narrow.remove(&(
            Some("Average grade (grams per tonne)".to_string()),
            "Processed".to_string(),
        ));
        let read =
            read_mine_stats(&workbook(fixture(false)), "2030Q2", Some(&narrow)).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(
            read.refusals[0].locator, "2030Q2:Minestats1!A8",
            "a sub-row is keyed by its section too"
        );
    }

    /// A unit the reader does not know refuses by locator: a summary header in ounces rather than thousands,
    /// a summary header naming no field, and (under the vocabulary) a `Minestats*` label in millions.
    #[test]
    fn test_unknown_unit_is_refused_by_locator() {
        let (ms, mut qtd) = fixture(false);
        put(&mut qtd, 2, 4, t("Gold produced (oz attributable)1"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1, "{:?}", read.refusals);
        assert_eq!(read.refusals[0].locator, "2030Q2:QTD!B4");
        assert!(
            read.refusals[0].reason.contains("not its unit \"000s oz\""),
            "{}",
            read.refusals[0].reason
        );
        assert!(
            read.values
                .iter()
                .all(|v| !(v.view == View::Summary && v.locator.ends_with("!B5"))),
            "the column is not emitted under a wrong unit"
        );

        let (ms, mut qtd) = fixture(false);
        put(&mut qtd, 9, 4, t("Gold output (000s oz attributable)"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1, "{:?}", read.refusals);
        assert_eq!(read.refusals[0].locator, "2030Q2:QTD!I4");
        assert!(read.refusals[0].reason.contains("names no known field"));

        let base = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        let vocabulary: LabelVocabulary = base
            .values
            .iter()
            .filter(|v| v.view == View::Minestats)
            .map(|v| (v.section.clone(), v.label.clone()))
            .collect();
        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 5, t("Tonnes processed (millions)"));
        let read =
            read_mine_stats(&workbook((ms, qtd)), "2030Q2", Some(&vocabulary)).expect("read");
        assert_eq!(read.refusals.len(), 1, "{:?}", read.refusals);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!A5");
    }
}
