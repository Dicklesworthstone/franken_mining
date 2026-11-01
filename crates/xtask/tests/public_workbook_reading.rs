//! Cross-crate workbook reading checks through the public APIs.
//! Fixtures and assertions are relocated from `fmine-public`; private parser tests remain there.
//! Public assurance display/refusal and mutation checks are retained in `public_report_contract.rs`.

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
