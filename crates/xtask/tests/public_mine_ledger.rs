//! Cross-crate Mine Statistics ledger tests: synthetic XLSX bytes through the public reader and ledger.
//! Moved unchanged from fmine-public's mine_ledger unit module; private numerical tests stay beside it.

use fmine_io::xlsx::Workbook;
use fmine_public::key::{StatusKind, SubjectStatus};
use fmine_public::mine_ledger::{
    Component, Family, Group, Instance, LedgerFile, LedgerRules, MineLedger, Need, Reason, State,
    mine_ledger, range_text,
};
use fmine_public::mine_stats::MineStatsRead;
use fmine_public::sources::{RegionMemberRow, RegisteredShare};
use fmine_registry::sources::MineStatsLabelAlias;

#[cfg(test)]
mod tests {
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};

    use super::*;
    use fmine_public::mine_stats::read_mine_stats;

    /// A summary row: the label, then 100% production, attributable production and attributable sold for the
    /// release's own year (columns B–D). A cell is `""` for blank, `—` or `n/a` as printed, a number as its
    /// stored text under General, or `#` (`##`) and the stored text for a number shown under `0.0` (`0.00`).
    type Row<'a> = (&'a str, [&'a str; 3]);

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

    /// A `Minestats*` sheet titled `title` with one three-month block: each site's name on row 2 over its two
    /// columns (from B), the block header on row 3 with 2030 and 2029 in each site's columns, then from row 4 one
    /// row per label with each site's current-year and prior-year cells (specs as in [`Row`]).
    fn minestats(
        name: &str,
        title: &str,
        sites: &[&str],
        rows: &[(&str, Vec<&str>)],
    ) -> MinimalSheet {
        const COLS: [&str; 6] = ["B", "C", "D", "E", "F", "G"];
        let mut cells = vec![
            ("A1".to_string(), text(title)),
            (
                "A3".to_string(),
                text("For the three months ended March 31,"),
            ),
        ];
        for (k, site) in sites.iter().enumerate() {
            let (cur, prior) = (COLS[2 * k], COLS[2 * k + 1]);
            cells.push((format!("{cur}2"), text(site)));
            for (col, year) in [(cur, "2030"), (prior, "2029")] {
                let year = MinimalCell::Styled {
                    raw: year.to_string(),
                    xf: 0,
                };
                cells.push((format!("{col}3"), year));
            }
        }
        for (k, (label, values)) in rows.iter().enumerate() {
            let row = 4 + k;
            cells.push((format!("A{row}"), text(label)));
            for (col, v) in COLS.into_iter().zip(values) {
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

    const AU: &str = "Mine Statistics (Gold)";
    const CU: &str = "Mine Statistics (Copper)";

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

    fn site_at(l: &MineLedger, cell: &str) -> (State, Option<String>) {
        let i = at(l, Family::SiteSheet, &format!("2030Q1:QTD!{cell}"));
        (i.state.clone(), i.cells.get(1).cloned().flatten())
    }

    fn ms(cell: &str) -> Option<String> {
        Some(format!("2030Q1:Minestats1!{cell}"))
    }

    /// Family 7: a summary value pairs with its site's print of the same row label (matched in lower case,
    /// whitespace collapsed, footnote stripped), period and year. 90 and 92 are boxes [89.5, 90.5] and
    /// [91.5, 92.5]: (iii), posed jointly, either print alone a correction. A subject with no column on a site
    /// sheet of the metal poses none, nor does the 100% column.
    #[test]
    fn test_site_sheet_pairs_each_summary_value_with_its_site_print() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[
                ("Hemlo", ["", "100", "90"]),
                ("Turquoise Ridge", ["", "50", "45"]),
            ],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo2"],
                &[
                    ("Production (Thousands of  ounces)", vec!["100", "80"]),
                    ("Sales (thousands of ounces)1", vec!["92", "85"]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(site_at(&l, "C5"), (State::Consistent, ms("B4")));
        assert_eq!(site_at(&l, "D5"), (State::Discrepancy, ms("B5")));
        assert_eq!(
            l.instances
                .iter()
                .filter(|i| i.family == Family::SiteSheet)
                .count(),
            2
        );
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert_eq!(c.id, "2030Q1:Minestats1!B5");
        assert_eq!(
            at(&l, Family::SiteSheet, "2030Q1:QTD!D5")
                .component
                .as_deref(),
            Some(c.id.as_str())
        );
        assert!(c.minimum && !c.joint_only);
        assert_eq!(sets(c), [["2030Q1:Minestats1!B5"], ["2030Q1:QTD!D5"]]);
        assert_eq!(l.components, 2);
    }

    /// The nil token against a number reads as number = 0 and is decided alone (E116's shape): 5 is [4.5, 5.5],
    /// (iii); 0 is [−0.5, 0.5], (i). Two tokens are not published; `n/a` against a number is refused at the `n/a`.
    #[test]
    fn test_site_sheet_nil_token_and_placeholders() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[
                ("Hemlo", ["", "5", "0"]),
                ("Tongon (89.7%)", ["", "—", "12"]),
            ],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo", "Tongon (89.7%)"],
                &[
                    ("Production (thousands of ounces)", vec!["—", "", "n/a", ""]),
                    ("Sales (thousands of ounces)", vec!["— ", "", "n/a", ""]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(site_at(&l, "C5"), (State::Discrepancy, ms("B4")));
        assert_eq!(site_at(&l, "D5"), (State::Consistent, ms("B5")));
        for cell in ["C5", "D5"] {
            let i = at(&l, Family::SiteSheet, &format!("2030Q1:QTD!{cell}"));
            assert!(i.nil_token && i.component.is_none(), "{cell}");
        }
        assert_eq!(site_at(&l, "C6"), (State::NotPublished, ms("D4")));
        assert_eq!(
            site_at(&l, "D6").0,
            refused(Reason::PlaceholderOperand, Some("2030Q1:Minestats1!D5"))
        );
        // Nothing is posed jointly.
        assert_eq!((l.components, l.infeasible.len()), (0, 0));
    }

    /// Only `"—"` and `"— "` are the nil token here: another spelling of it is unreadable, never read as zero.
    #[test]
    fn test_other_nil_spelling_is_unreadable() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[("Hemlo", ["", "5", "0"])],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo"],
                &[
                    ("Production (thousands of ounces)", vec!["  —", ""]),
                    ("Sales (thousands of ounces)", vec!["—", ""]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(
            site_at(&l, "C5"),
            (
                refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B4")),
                ms("B4")
            )
        );
        assert_eq!(site_at(&l, "D5").0, State::Consistent);
    }

    /// The census's order: a subject on two site sheets of one metal; a withheld subject (its counterpart still
    /// named); a missing row; then a blank or unreadable operand at that cell.
    #[test]
    fn test_site_sheet_refusals_in_order() {
        let statuses = loulo_withheld();
        let (l, refusals) = site_ledger(
            GOLD,
            &[
                ("Hemlo", ["", "100", "90"]),
                ("Loulo-Gounkoto (80%)", ["", "50", "40"]),
                ("Kibali (45%)", ["", "30", "20"]),
                ("Tongon (89.7%)", ["", "60", "55"]),
            ],
            vec![
                minestats(
                    "Minestats1",
                    AU,
                    &["Hemlo", "Loulo-Gounkoto (80%)"],
                    &[
                        ("Production (thousands of ounces)", vec!["", "", "50", ""]),
                        ("Sales (thousands of ounces)", vec!["90.4", "", "40", ""]),
                    ],
                ),
                minestats(
                    "Minestats2",
                    AU,
                    &["Kibali (45%)", "Tongon (89.7%)"],
                    &[("Production (thousands of ounces)", vec!["30", "", "60", ""])],
                ),
                minestats(
                    "Minestats3",
                    AU,
                    &["Kibali (45%)"],
                    &[("Production (thousands of ounces)", vec!["30", ""])],
                ),
            ],
            &LedgerRules {
                statuses: &statuses,
                ..NONE
            },
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        let blank = refused(Reason::BlankOperand, Some("2030Q1:Minestats1!B4"));
        assert_eq!(site_at(&l, "C5"), (blank, ms("B4")));
        // General shows 90.4 by column width: unreadable, never guessed.
        let unreadable = refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B5"));
        assert_eq!(site_at(&l, "D5"), (unreadable, ms("B5")));
        let withheld = refused(
            Reason::Withheld {
                evidence: "E818".to_string(),
            },
            None,
        );
        assert_eq!(site_at(&l, "C6"), (withheld.clone(), ms("D4")));
        assert_eq!(site_at(&l, "D6"), (withheld, ms("D5")));
        for cell in ["C7", "D7"] {
            assert_eq!(
                site_at(&l, cell),
                (refused(Reason::OperandUnreadable, None), None)
            );
        }
        assert_eq!(
            site_at(&l, "C8"),
            (State::Consistent, Some("2030Q1:Minestats2!D4".to_string()))
        );
        assert_eq!(
            site_at(&l, "D8"),
            (refused(Reason::OperandRowMissing, None), None)
        );
    }

    /// W1-DR-02: a registered alias reads a stale row label as the registered one, for its releases only, and
    /// converts nothing. The stale row beside the registered one is one label printed twice: refused at the first.
    #[test]
    fn test_label_alias_reads_a_stale_row_for_its_releases() {
        let alias = |what: &str, first: &str, last: &str| MineStatsLabelAlias {
            label: format!("{what} (millions of pounds)"),
            reads_as: format!("{what} (Thousands of tonnes)"),
            first: first.to_string(),
            last: last.to_string(),
            evidence: "W1-DR-02".to_string(),
        };
        let states = |aliases: &[MineStatsLabelAlias], both: bool| {
            let mut rows = vec![
                ("Production (millions of pounds)", vec!["30", ""]),
                ("Sales (millions of pounds)", vec!["29", ""]),
            ];
            if both {
                rows.push(("Production (thousands of tonnes)", vec!["30", ""]));
            }
            let (l, refusals) = site_ledger(
                COPPER,
                &[("Lumwana", ["", "30", "29"])],
                vec![minestats("Minestats1", CU, &["Lumwana"], &rows)],
                &LedgerRules { aliases, ..NONE },
            );
            assert!(refusals.is_empty(), "{refusals:?}");
            [site_at(&l, "C5"), site_at(&l, "D5")]
        };
        let missing = (refused(Reason::OperandRowMissing, None), None);
        assert_eq!(states(&[], false), [missing.clone(), missing.clone()]);
        let now = [
            alias("Production", "2030Q1", "2030Q4"),
            alias("Sales", "2030Q1", "2030Q4"),
        ];
        assert_eq!(
            states(&now, false),
            [(State::Consistent, ms("B4")), (State::Consistent, ms("B5"))]
        );
        let later = [
            alias("Production", "2031Q1", "2031Q4"),
            alias("Sales", "2031Q1", "2031Q4"),
        ];
        assert_eq!(states(&later, false), [missing.clone(), missing]);
        let [production, sales] = states(&now, true);
        assert_eq!(
            production,
            (
                refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B4")),
                ms("B4")
            )
        );
        assert_eq!(sales, (State::Consistent, ms("B5")));
    }

    /// A row label printed twice in one block: the reader refuses the second row, and the ledger refuses the
    /// pair at the first rather than take one print as the label's only one.
    #[test]
    fn test_label_printed_twice_is_refused_at_the_first() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[("Hemlo", ["", "100", "90"])],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo"],
                &[
                    ("Production (thousands of ounces)", vec!["100", ""]),
                    ("Sales (thousands of ounces)", vec!["90", ""]),
                    ("Production (thousands of ounces)", vec!["101", ""]),
                ],
            )],
            &NONE,
        );
        assert_eq!(refusals, ["2030Q1:Minestats1!A6"]);
        assert_eq!(
            site_at(&l, "C5"),
            (
                refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B4")),
                ms("B4")
            )
        );
        assert_eq!(site_at(&l, "D5"), (State::Consistent, ms("B5")));
    }

    /// The site print joins the ownership identity's component. Alone each holds: C = B/2 in [99.75, 100.25]
    /// meets C's [99.5, 100.5], and 100 meets 101 at 100.5. Together C is 100.5 and B = 201 misses
    /// [199.5, 200.5]. Disregarding B leaves C = 100.5 (B in [201, 201]); disregarding the site print leaves
    /// C in [99.75, 100.25] (the print in [399/4, 401/4]); disregarding C leaves the two in conflict.
    #[test]
    fn test_site_print_joins_the_ownership_component() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[("Carlin (50%)", ["200", "100", "90"])],
            vec![minestats(
                "Minestats1",
                AU,
                &["Carlin (50%)"],
                &[
                    ("Production (thousands of ounces)", vec!["101", ""]),
                    ("Sales (thousands of ounces)", vec!["90", ""]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(
            at(&l, Family::Ownership, "2030Q1:QTD!C5").state,
            State::Consistent
        );
        assert_eq!(site_at(&l, "C5"), (State::Consistent, ms("B4")));
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert!(c.joint_only && c.minimum);
        assert_eq!(c.id, "2030Q1:Minestats1!B4");
        assert_eq!(c.instances.len(), 2);
        assert_eq!(sets(c), [["2030Q1:Minestats1!B4"], ["2030Q1:QTD!B5"]]);
        let ranges: Vec<(&str, String)> = c
            .corrections
            .iter()
            .flat_map(|s| &s.implied)
            .map(|(cell, r)| (cell.as_str(), range_text(r)))
            .collect();
        assert_eq!(
            ranges,
            [
                ("2030Q1:Minestats1!B4", "[399/4, 401/4]".to_string()),
                ("2030Q1:QTD!B5", "[201, 201]".to_string()),
            ]
        );
        assert_eq!(l.components, 2);
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

    /// The gold flows (B–D) and unit costs per ounce (E–G).
    const GOLD_RATES: [&str; 6] = [
        GOLD[0],
        GOLD[1],
        GOLD[2],
        "Cost of sales ($/oz)",
        "Total cash costs ($/oz)",
        "All-in sustaining costs ($/oz)",
    ];

    /// A rate row: the label, then 100% and attributable production, attributable sold, cost of sales, TCC and
    /// AISC per ounce (specs as in [`Row`]).
    type RateRow<'a> = (&'a str, [&'a str; 6]);

    /// Release 2030Q1: one `QTD` sheet of [`GOLD_RATES`], rows from row 5.
    fn rate_ledger(rows: &[RateRow<'_>], rules: &LedgerRules<'_>) -> MineLedger {
        let rows: Vec<(&str, &[&str])> = rows.iter().map(|(l, v)| (*l, v.as_slice())).collect();
        let sheet = summary_cols("QTD", "three", &GOLD_RATES, &rows);
        let (l, refusals) = run(vec![("2030Q1".to_string(), vec![sheet])], rules);
        assert!(refusals.is_empty(), "{refusals:?}");
        l
    }

    fn rate_at<'a>(l: &'a MineLedger, cell: &str) -> &'a Instance {
        at(l, Family::SoldWeightedRate, &format!("2030Q1:QTD!{cell}"))
    }

    /// North America (row 7) = Carlin (row 5) and Hemlo (row 6), 100 ounces each.
    fn north_america<'a>(
        carlin: [&'a str; 6],
        hemlo: [&'a str; 6],
        na: [&'a str; 6],
    ) -> Vec<RateRow<'a>> {
        vec![
            ("Carlin (50%)", carlin),
            ("Hemlo (100%)", hemlo),
            ("North America", na),
        ]
    }

    const CARLIN: [&str; 6] = ["200", "100", "100", "1000", "800", "1300"];
    const HEMLO: [&str; 6] = ["100", "100", "100", "1500", "1200", "1700"];

    #[test]
    fn test_region_rate_is_its_members_sold_weighted_mean() {
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
        // Weights in [99.5, 100.5] and rates within 0.5 put the true mean of 1,000 and 1,500 in [1,248.25,
        // 1,251.75], derived by hand: a printed region rate meets it from 1,248 to 1,252.
        for (printed, code) in [
            ("1247", "iv"),
            ("1248", "i"),
            ("1250", "i"),
            ("1252", "i"),
            ("1253", "iv"),
        ] {
            let na = ["300", "200", "200", printed, "1000", "1600"];
            let l = rate_ledger(&north_america(CARLIN, HEMLO, na), &rules);
            assert_eq!(
                rate_at(&l, "E7").state.code(),
                code,
                "cost of sales {printed}"
            );
        }
        let na = ["300", "200", "200", "1250", "1000", "1600"];
        let l = rate_ledger(&north_america(CARLIN, HEMLO, na), &rules);
        let cos = rate_at(&l, "E7");
        assert_eq!(
            (cos.subject.as_str(), cos.field.as_str(), cos.nil_token),
            ("North America", "cos_usd_per_oz", false)
        );
        // Each member's rate then its sold, in row order, then the region's rate.
        assert_eq!(
            cos.cells,
            locs(&[Some("E5"), Some("D5"), Some("E6"), Some("D6"), Some("E7")])
        );
        assert_eq!(rate_at(&l, "F7").state, State::Consistent);
        // 1,600 is far from the mean of 1,300 and 1,700: (iv), never (iii), and never posed jointly.
        let aisc = rate_at(&l, "G7");
        assert_eq!(aisc.state, State::Unresolved(Need::CostTotals));
        assert_eq!(aisc.component, None);
        assert!(l.infeasible.is_empty());
        assert_eq!(l.count(Family::SoldWeightedRate, "iii"), 0);
        // The registry's member order does not set the operand order; the rows do.
        let reversed = [members[1].clone(), members[0].clone()];
        let l = rate_ledger(
            &north_america(CARLIN, HEMLO, na),
            &LedgerRules {
                region_members: &reversed,
                ..rules
            },
        );
        assert_eq!(rate_at(&l, "E7").cells, cos.cells);
    }

    #[test]
    fn test_weighted_rate_nil_sold_placeholders_and_weights() {
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
        let cos = |carlin: [&str; 6], hemlo: [&str; 6], na_cos: &str| {
            let na = ["300", "200", "200", na_cos, "1000", "1600"];
            let i = rate_at(
                &rate_ledger(&north_america(carlin, hemlo, na), &rules),
                "E7",
            )
            .clone();
            (i.state, i.nil_token)
        };
        let at = |c: &str| Some(format!("2030Q1:QTD!{c}"));
        let refused = |reason, cell: Option<String>| State::Refused { reason, at: cell };
        // Hemlo sold nothing: it leaves the identity whatever its rate prints, and Carlin's rate alone decides.
        let idle = ["—", "—", "—", "1500", "—", "n/a"];
        assert_eq!(cos(CARLIN, idle, "1000"), (State::Consistent, true));
        assert_eq!(
            cos(CARLIN, idle, "1250"),
            (State::Unresolved(Need::CostTotals), true)
        );
        // A rate is never 0: a nil rate beside ounces sold refuses.
        let no_rate = ["100", "100", "100", "—", "1200", "1700"];
        assert_eq!(
            cos(CARLIN, no_rate, "1250"),
            (refused(Reason::PlaceholderOperand, at("E6")), false)
        );
        // Nobody sold: no weighted mean to compare.
        let carlin_idle = ["—", "—", "—", "1000", "800", "1300"];
        assert_eq!(
            cos(carlin_idle, idle, "1250"),
            (refused(Reason::WeightsAllNil, None), false)
        );
        // Zero ounces is not the nil token: its box reaches below 0.
        let zero = ["0", "0", "0", "1500", "1200", "1700"];
        assert_eq!(
            cos(CARLIN, zero, "1250"),
            (refused(Reason::WeightNotPositive, at("D6")), false)
        );
        let negative = ["0", "0", "-5", "1500", "1200", "1700"];
        assert_eq!(
            cos(CARLIN, negative, "1250"),
            (refused(Reason::WeightNotPositive, at("D6")), false)
        );
        // A blank comes first; all placeholders are not published.
        let blank = ["100", "100", "100", "", "1200", "1700"];
        assert_eq!(
            cos(CARLIN, blank, "1250"),
            (refused(Reason::BlankOperand, at("E6")), false)
        );
        let none = ["—", "—", "—", "—", "—", "—"];
        assert_eq!(cos(none, none, "—"), (State::NotPublished, false));
    }

    #[test]
    fn test_weighted_rate_refusals_by_name_and_row() {
        let regions = ["North America".to_string(), "Africa".to_string()];
        let rows = vec![
            ("Nevada Gold Mines LLC (50%)", CARLIN),
            ("Hemlo (100%)", HEMLO),
            (
                "North America",
                ["300", "200", "200", "1250", "1000", "1600"],
            ),
            ("Africa", ["10", "5", "5", "900", "700", "1100"]),
        ];
        let base = [
            member("North America", "Nevada Gold Mines", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
            member("Africa", "Kibali", "2031Q1"),
        ];
        let labels = [RegisteredShare {
            subject: "Nevada Gold Mines".to_string(),
            num: 1,
            den: 2,
            evidence: "E1".to_string(),
            label: "Nevada Gold Mines LLC".to_string(),
        }];
        let ledger_with = |rows: &[RateRow<'_>], members: &[RegionMemberRow], labels, statuses| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: members,
                labels,
                statuses,
                ..NONE
            };
            rate_ledger(rows, &rules)
        };
        let l = ledger_with(&rows, &base, &labels, &[]);
        // Found by its registered label; Africa's members join the map in 2031.
        assert_eq!(rate_at(&l, "E7").state, State::Consistent);
        let africa = rate_at(&l, "E8");
        assert_eq!(africa.state, refused(Reason::RegionNotInMap, None));
        assert_eq!(africa.cells, locs(&[Some("E8")]));
        // Without the label the member has no row; the cells hold the members found.
        let missing = rate_at(&ledger_with(&rows, &base, &[], &[]), "E7").clone();
        assert_eq!(missing.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(missing.cells, locs(&[Some("E6"), Some("D6"), Some("E7")]));
        // Printed under both names: refused at the rate cell of the row printed first.
        let mut twice = rows.clone();
        twice.insert(0, ("Nevada Gold Mines (50%)", CARLIN));
        let both = rate_at(&ledger_with(&twice, &base, &labels, &[]), "E8").clone();
        assert_eq!(
            both.state,
            refused(Reason::OperandUnreadable, Some("2030Q1:QTD!E5"))
        );
        // A withheld member excludes the region for the periods its status covers.
        let statuses = [SubjectStatus {
            subject: "Hemlo".to_string(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "E818".to_string(),
        }];
        let held = ledger_with(&rows, &base, &labels, &statuses);
        assert_eq!(
            rate_at(&held, "E7").state,
            refused(
                Reason::Withheld {
                    evidence: "E818".to_string()
                },
                None
            )
        );
    }

    /// One block of a site sheet: its months word and its `(label, cells)` rows.
    type SiteBlock<'a> = (&'a str, Vec<(&'a str, Vec<&'a str>)>);

    /// A `Minestats*` sheet titled `title` with one block per `(months, rows)`: each block's site names over
    /// their two columns (from B: 2030, then 2029), the block header below them, then one row per label with
    /// each site's current-year and prior-year cells. The first block's site row is row 2.
    fn site_blocks(
        name: &str,
        title: &str,
        sites: &[&str],
        blocks: &[SiteBlock<'_>],
    ) -> MinimalSheet {
        const COLS: [&str; 14] = [
            "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O",
        ];
        let mut cells = vec![("A1".to_string(), text(title))];
        let mut row = 2;
        for (months, rows) in blocks {
            for (k, site) in sites.iter().enumerate() {
                let (cur, prior) = (COLS[2 * k], COLS[2 * k + 1]);
                cells.push((format!("{cur}{row}"), text(site)));
                for (col, year) in [(cur, "2030"), (prior, "2029")] {
                    let year = MinimalCell::Styled {
                        raw: year.to_string(),
                        xf: 0,
                    };
                    cells.push((format!("{col}{}", row + 1), year));
                }
            }
            cells.push((
                format!("A{}", row + 1),
                text(&format!("For the {months} months ended September 30,")),
            ));
            row += 2;
            for (label, values) in rows {
                cells.push((format!("A{row}"), text(label)));
                for (col, v) in COLS.into_iter().zip(values) {
                    if let Some(c) = data(v) {
                        cells.push((format!("{col}{row}"), c));
                    }
                }
                row += 1;
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    /// Release `2030Q{n}`: empty summary sheets beside the given `Minestats*` sheets.
    fn site_release(n: u8, sheets: Vec<MinimalSheet>) -> (String, Vec<MinimalSheet>) {
        let mut all = vec![summary("QTD", "three", GOLD, &[])];
        if n > 1 {
            let months = ["three", "six", "nine", "twelve"][usize::from(n) - 1];
            all.push(summary("YTD", months, GOLD, &[]));
        }
        all.extend(sheets);
        (format!("2030Q{n}"), all)
    }

    fn site_sums(l: &MineLedger) -> Vec<&Instance> {
        l.instances
            .iter()
            .filter(|i| i.family == Family::SiteQuarterSum)
            .collect()
    }

    fn loc(release: u8, cell: &str) -> Option<String> {
        Some(format!("2030Q{release}:Minestats1!{cell}"))
    }

    /// BD-331.S4 family 2 on the site sheets: H1 = Q1 + Q2 across the Q1 and Q2 releases, and 9M = H1 + Q3, on the
    /// additive rows only. `Underground` under `Tonnes mined` is posed and under `Cost per tonne mined` is not;
    /// strip ratio and unit costs pose none. Every value is a whole number, so a sum of two holds within 1½. By
    /// hand: H1 tonnes 100 + 120 = 220 and 90 + 80 = 170 hold; open-pit ore 60 + 70 = 130 holds and 50 + 45 = 95
    /// against 97 does not (iii); 9M tonnes 220 + 130 = 350 and 170 + 100 = 270 hold, 22 + 13 = 35 against 36
    /// holds (the bound 1½ not reached); 9M open-pit ore has no Q3 row (v).
    #[test]
    fn test_site_quarter_sums_across_releases_on_additive_rows() {
        let labels = [
            "Tonnes mined (thousands)",
            "Open Pit Ore",
            "Underground",
            "Strip Ratio",
            "Cost per tonne mined ($/tonne)",
            "Underground",
            "Production (thousands of ounces)",
        ];
        let block = |values: [[&'static str; 2]; 7]| -> Vec<(&'static str, Vec<&'static str>)> {
            labels
                .into_iter()
                .zip(values)
                .map(|(l, v)| (l, v.to_vec()))
                .collect()
        };
        let q1 = block([
            ["100", "90"],
            ["60", "50"],
            ["40", "40"],
            ["2", "2"],
            ["3", "3"],
            ["30", "30"],
            ["10", "9"],
        ]);
        let q2 = block([
            ["120", "80"],
            ["70", "45"],
            ["50", "35"],
            ["2", "2"],
            ["3", "3"],
            ["31", "31"],
            ["12", "8"],
        ]);
        let h1 = block([
            ["220", "170"],
            ["130", "97"],
            ["90", "75"],
            ["2", "2"],
            ["3", "3"],
            ["61", "61"],
            ["22", "17"],
        ]);
        let q3 = vec![
            ("Tonnes mined (thousands)", vec!["130", "100"]),
            ("Production (thousands of ounces)", vec!["13", "10"]),
        ];
        let nine = vec![
            ("Tonnes mined (thousands)", vec!["350", "270"]),
            ("Open Pit Ore", vec!["200", "150"]),
            ("Production (thousands of ounces)", vec!["36", "27"]),
        ];
        let sheet = |blocks| site_blocks("Minestats1", AU, &["Site A"], blocks);
        let (l, refusals) = run(
            vec![
                site_release(1, vec![sheet(&[("three", q1)])]),
                site_release(2, vec![sheet(&[("three", q2), ("six", h1)])]),
                site_release(3, vec![sheet(&[("three", q3), ("nine", nine)])]),
            ],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        let sums = site_sums(&l);
        // H1: four additive rows × two years; 9M: three rows × two years.
        assert_eq!(sums.len(), 14);
        let tonnes = at(&l, Family::SiteQuarterSum, "2030Q2:Minestats1!B13");
        assert_eq!(
            (tonnes.field.as_str(), tonnes.group, tonnes.state.code()),
            ("au_tonnes_mined_kt", Group::Current, "i")
        );
        assert_eq!(tonnes.cells, [loc(1, "B4"), loc(2, "B4"), loc(2, "B13")]);
        let ore = at(&l, Family::SiteQuarterSum, "2030Q2:Minestats1!C14");
        assert_eq!(
            (ore.field.as_str(), ore.group, &ore.state),
            (
                "au_tonnes_mined_open_pit_ore_kt",
                Group::Prior,
                &State::Discrepancy
            )
        );
        assert_eq!(ore.cells, [loc(1, "C5"), loc(2, "C5"), loc(2, "C14")]);
        assert!(ore.component.is_none() && !ore.nil_token);
        // The tonnes row of Underground (row 15) is posed; the unit-cost row (18) is not.
        let underground = at(&l, Family::SiteQuarterSum, "2030Q2:Minestats1!B15");
        assert_eq!(underground.field, "au_tonnes_mined_underground_kt");
        assert!(sums.iter().all(|i| !i.anchor.ends_with("18")
            && !i.anchor.ends_with("16")
            && !i.anchor.ends_with("17")));
        let nine_tonnes = at(&l, Family::SiteQuarterSum, "2030Q3:Minestats1!B8");
        assert_eq!(
            nine_tonnes.cells,
            [loc(2, "B13"), loc(3, "B4"), loc(3, "B8")]
        );
        assert_eq!(nine_tonnes.state, State::Consistent);
        assert_eq!(
            at(&l, Family::SiteQuarterSum, "2030Q3:Minestats1!B10").state,
            State::Consistent
        );
        let nine_ore = at(&l, Family::SiteQuarterSum, "2030Q3:Minestats1!B9");
        assert_eq!(nine_ore.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(nine_ore.cells, [loc(2, "B14"), None, loc(3, "B9")]);
        assert_eq!(
            ["i", "iii", "v", "np"].map(|c| l.count(Family::SiteQuarterSum, c)),
            [11, 1, 2, 0]
        );
    }

    /// The nil token is exactly 0 (a site that began in Q2), all nil is not published, `n/a` and a blank refuse
    /// at their cell, a withheld site is excluded by name for the periods its status covers, and a site that moves
    /// to another sheet is still found. Sites (current-year cells Q1, Q2, H1): Nil (—, 12, 12) holds; Off (—, 12,
    /// 14) misses by 2 with slack 1, (iii); Idle all —; Na (n/a, 12, 12); Gap (12, blank, 24); Loulo-Gounkoto
    /// withheld for January–March 2030 (a status's bounds are months), so H1 is refused for its Q1 operand
    /// though H1 itself is not inside the status; Moved on Minestats2 in Q1. Every prior-year sum is 1 + 1 = 2
    /// and holds.
    #[test]
    fn test_site_quarter_sum_nil_placeholders_withheld_and_moved() {
        let sites = ["Nil", "Off", "Idle", "Na", "Gap", "Loulo-Gounkoto", "Moved"];
        let prod = "Production (thousands of ounces)";
        let row = |cur: [&'static str; 7], prior: &'static str| -> Vec<(&str, Vec<&'static str>)> {
            vec![(prod, cur.into_iter().flat_map(|c| [c, prior]).collect())]
        };
        let q1 = row(["—", "—", "—", "n/a", "12", "10", "10"], "1");
        let q2 = row(["12", "12", "—", "12", "", "10", "10"], "1");
        let h1 = row(["12", "14", "—", "12", "24", "20", "20"], "2");
        let six = &sites[..6];
        let statuses = loulo_withheld();
        let rules = LedgerRules {
            statuses: &statuses,
            ..NONE
        };
        let q1_main: Vec<(&str, Vec<&str>)> = vec![(prod, q1[0].1[..12].to_vec())];
        let q1_moved: Vec<(&str, Vec<&str>)> = vec![(prod, q1[0].1[12..].to_vec())];
        let (l, refusals) = run(
            vec![
                site_release(
                    1,
                    vec![
                        site_blocks("Minestats1", AU, six, &[("three", q1_main)]),
                        site_blocks("Minestats2", AU, &["Moved"], &[("three", q1_moved)]),
                    ],
                ),
                site_release(
                    2,
                    vec![site_blocks(
                        "Minestats1",
                        AU,
                        &sites,
                        &[("three", q2), ("six", h1)],
                    )],
                ),
            ],
            &rules,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        // Q2 block: data row 4; H1 block: site row 5, header 6, data row 7. Site k's current column is B, D, F, …
        let h1_at = |col: &str| {
            at(
                &l,
                Family::SiteQuarterSum,
                &format!("2030Q2:Minestats1!{col}7"),
            )
        };
        let nil = h1_at("B");
        assert_eq!((&nil.state, nil.nil_token), (&State::Consistent, true));
        assert_eq!(nil.cells, [loc(1, "B4"), loc(2, "B4"), loc(2, "B7")]);
        let off = h1_at("D");
        assert_eq!((&off.state, off.nil_token), (&State::Discrepancy, true));
        assert_eq!(h1_at("F").state, State::NotPublished);
        assert_eq!(
            h1_at("H").state,
            refused(Reason::PlaceholderOperand, loc(1, "H4").as_deref())
        );
        assert_eq!(
            h1_at("J").state,
            refused(Reason::BlankOperand, loc(2, "J4").as_deref())
        );
        assert_eq!(
            h1_at("L").state,
            refused(
                Reason::Withheld {
                    evidence: "E818".to_string()
                },
                None
            )
        );
        let moved = h1_at("N");
        assert_eq!(moved.state, State::Consistent);
        assert_eq!(
            moved.cells,
            [
                Some("2030Q1:Minestats2!B4".to_string()),
                loc(2, "N4"),
                loc(2, "N7")
            ]
        );
        // Every prior-year sum holds, Loulo-Gounkoto's too: its status does not cover 2029.
        assert!(
            site_sums(&l)
                .iter()
                .filter(|i| i.group == Group::Prior)
                .all(|i| i.state == State::Consistent && !i.nil_token)
        );
        assert_eq!(
            ["i", "iii", "v", "np"].map(|c| l.count(Family::SiteQuarterSum, c)),
            [9, 1, 3, 1]
        );
    }

    /// The refusals that come before the operands: a missing previous release, a site with columns on two sheets
    /// of its metal in an operand's release (its cells there not located), and a row the previous release does
    /// not print.
    #[test]
    fn test_site_quarter_sum_refusals_before_the_operands() {
        let prod = "Production (thousands of ounces)";
        let sales = "Sales (thousands of ounces)";
        let quarter = vec![(prod, vec!["10", "9"])];
        let both = vec![(prod, vec!["10", "9"]), (sales, vec!["10", "9"])];
        let half = vec![(prod, vec!["20", "18"]), (sales, vec!["20", "18"])];
        let (alone, _) = run(
            vec![site_release(
                2,
                vec![site_blocks(
                    "Minestats1",
                    AU,
                    &["Site A"],
                    &[("three", both.clone()), ("six", half.clone())],
                )],
            )],
            &NONE,
        );
        let sums = site_sums(&alone);
        assert_eq!(sums.len(), 4);
        assert!(
            sums.iter()
                .all(|i| i.state == refused(Reason::ReleaseMissing, None) && i.cells[0].is_none())
        );
        let (l, refusals) = run(
            vec![
                site_release(
                    1,
                    vec![site_blocks(
                        "Minestats1",
                        AU,
                        &["Site A", "Site D"],
                        &[("three", vec![(prod, vec!["10", "9", "10", "9"])])],
                    )],
                ),
                site_release(
                    2,
                    vec![
                        site_blocks(
                            "Minestats1",
                            AU,
                            &["Site A", "Site D"],
                            &[
                                (
                                    "three",
                                    vec![
                                        (prod, vec!["10", "9", "10", "9"]),
                                        (sales, vec!["10", "9", "10", "9"]),
                                    ],
                                ),
                                (
                                    "six",
                                    vec![
                                        (prod, vec!["20", "18", "20", "18"]),
                                        (sales, vec!["20", "18", "20", "18"]),
                                    ],
                                ),
                            ],
                        ),
                        site_blocks("Minestats2", AU, &["Site D"], &[("three", quarter)]),
                    ],
                ),
            ],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        // Minestats1, Q2 block rows 4–5, H1 block rows 8–9; Site A in B/C, Site D in D/E.
        let a_prod = at(&l, Family::SiteQuarterSum, "2030Q2:Minestats1!B8");
        assert_eq!(a_prod.state, State::Consistent);
        let a_sales = at(&l, Family::SiteQuarterSum, "2030Q2:Minestats1!B9");
        assert_eq!(a_sales.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(a_sales.cells, [None, loc(2, "B5"), loc(2, "B9")]);
        let d_prod = at(&l, Family::SiteQuarterSum, "2030Q2:Minestats1!D8");
        assert_eq!(d_prod.state, refused(Reason::OperandUnreadable, None));
        assert_eq!(d_prod.cells, [loc(1, "D4"), None, loc(2, "D8")]);
    }
}
