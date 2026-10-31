//! bd-4wvl.3 (BD-340.S3): blame on the registered Barrick Mine Stats known answers E113–E116.
//!
//! Every fixture cell cites `<file>:<sheet>!<cell>` and the file's registered sha256. Values are those quoted in
//! `planning/corpus/C1_barrick_public_stats.md` (E113, E114, E116) plus E115's four cells, which the evidence row
//! does not give and which were captured from the registered bytes: reader one is a standard-library XML read at
//! capture time, reader two is `test_blame_fixtures_match_registered_bytes` through `fmine-io` (ignored here; it
//! runs in BD-339's real-file job). Ownership basis: column B is "100% basis", column C "attributable share" (gold
//! in 000s oz in rows 5 and 7, copper in kt in row 28); E10 and E11 are cost of sales in $/oz.
//!
//! Identities: a half year is the sum of its quarters, each figure rounded once (`RoundedOnce`), and an
//! attributable figure is the stated share of the 100% figure (`Scale`). The tie convention is `Closed`: any tie
//! may have rounded either way.

// In a module under `cfg(test)` so the helpers get clippy's test allowances (unwrap, expect, panic).
#[cfg(test)]
mod blame {
    use std::path::PathBuf;

    use fmine_io::xlsx::{CellValue, parse_a1_ref};
    use fmine_numerics::exact_feas::{
        Blame, BlameOutcome, Cell, Closure, ComponentBlame, Construction, Convention,
        CorrectionLabel, Identity, IdentityKind, Interval, Outcome, Placeholder, Rat, Rules, Scope,
        SignatureLabel, XfRefusal, blame, check_conflict_set, presence_conflicts, solve,
    };
    use fmine_public::load_registered_sources;
    use fmine_public::sources::load_issuer_profile;

    const Q1: &str = "Barrick_Q1_2026_Mine_Stats.xlsx";
    const Q2: &str = "Barrick_Q2_2026_Mine_Stats.xlsx";
    /// As registered in `registries/sources/barrick.toml`.
    const SHA256: [(&str, &str); 2] = [
        (
            Q1,
            "ddef08dd544f0df511fc30f53c1dd889bb6878257d394b8baad7ac3dbfa8c5d2",
        ),
        (
            Q2,
            "cf0d3f4fea9c96f967c138d0aca549f4ff4f4a233bd2c5946c4035bf96c3df8e",
        ),
    ];

    /// Every fixture cell: (evidence, file, sheet, cell, printed).
    const CELLS: [(&str, &str, &str, &str, i128); 14] = [
        ("E113", Q1, "QTD", "B5", 283),
        ("E113", Q1, "QTD", "C5", 173),
        ("E113", Q2, "QTD", "B5", 280),
        ("E113", Q2, "YTD", "B5", 561),
        ("E114", Q1, "QTD", "B7", 148),
        ("E114", Q2, "QTD", "B7", 130),
        ("E114", Q2, "YTD", "B7", 281),
        ("E114", Q2, "YTD", "C7", 171),
        ("E115", Q1, "QTD", "B28", 18),
        ("E115", Q2, "QTD", "B28", 14),
        ("E115", Q2, "QTD", "C28", 6),
        ("E115", Q2, "YTD", "B28", 30),
        ("E116", Q2, "YTD", "E10", 1691),
        ("E116", Q2, "YTD", "E11", 1691),
    ];
    /// Every fixture placeholder: (evidence, file, sheet, cell, token as stored).
    const PLACEHOLDERS: [(&str, &str, &str, &str, &str); 1] =
        [("E116", Q2, "Minestats1", "N103", "—")];

    fn sha256(file: &str) -> &'static str {
        SHA256
            .iter()
            .find(|(f, _)| *f == file)
            .map(|(_, s)| *s)
            .expect("a registered file")
    }

    fn loc(file: &str, sheet: &str, a1: &str) -> String {
        format!("{file}:{sheet}!{a1}")
    }

    fn q(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    fn int(n: i128) -> Rat {
        Rat::from_integer(n)
    }

    /// The fixture cells of one evidence id, with any printed value replaced.
    fn cells(evidence: &str, replace: &[(&str, i128)]) -> Vec<Cell> {
        CELLS
            .iter()
            .filter(|c| c.0 == evidence)
            .map(|&(_, file, sheet, a1, printed)| {
                let locator = loc(file, sheet, a1);
                let printed = replace
                    .iter()
                    .find(|(l, _)| *l == locator)
                    .map_or(printed, |(_, v)| *v);
                Cell {
                    sha256: sha256(file).to_string(),
                    token: printed.to_string(),
                    printed: int(printed),
                    step: int(1),
                    scope: Scope::Visible,
                    locator,
                }
            })
            .collect()
    }

    fn half_year(id: &str, h1: &str, q1: &str, q2: &str) -> Identity {
        Identity {
            id: id.to_string(),
            family: "half_year".to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Sum {
                total: h1.to_string(),
                lines: vec![q1.to_string(), q2.to_string()],
                signs: vec![1, 1],
            },
        }
    }

    fn share(id: &str, attributable: &str, full: &str, factor: Rat) -> Identity {
        Identity {
            id: id.to_string(),
            family: "share".to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Scale {
                target: attributable.to_string(),
                source: full.to_string(),
                factor,
            },
        }
    }

    fn rules() -> Rules {
        Rules::new(Convention::Closed).with("half_year", Construction::RoundedOnce)
    }

    fn closed(lo: Rat, hi: Rat) -> Interval {
        Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        }
    }

    /// E113 (Carlin, 61.5%): Q1 100% B5 283, attributable C5 173; Q2 B5 280; H1 B5 561.
    fn e113(replace: &[(&str, i128)]) -> (Vec<Cell>, Vec<Identity>) {
        (
            cells("E113", replace),
            vec![
                half_year(
                    "carlin_h1",
                    &loc(Q2, "YTD", "B5"),
                    &loc(Q1, "QTD", "B5"),
                    &loc(Q2, "QTD", "B5"),
                ),
                share(
                    "carlin_q1_share",
                    &loc(Q1, "QTD", "C5"),
                    &loc(Q1, "QTD", "B5"),
                    q(123, 200),
                ),
            ],
        )
    }

    fn the_blame(found: &[ComponentBlame]) -> &Blame {
        assert_eq!(found.len(), 1, "{found:?}");
        match &found[0].outcome {
            BlameOutcome::Blamed(b) => b,
            other => panic!("not blamed: {other:?}"),
        }
    }

    fn conflict_cells(b: &Blame) -> Vec<Vec<String>> {
        let mut sets: Vec<Vec<String>> = b.conflicts.iter().map(|c| c.cells.clone()).collect();
        sets.sort();
        sets
    }

    /// E113: the share fails ([173.74, 174.35] against [172.5, 173.5]) and the half year fails (Q1 + Q2 − H1 in
    /// [0.5, 3.5]). Only Q1's 100% figure is in both; with it disregarded, its range is [172.5/0.615, 173.5/0.615]
    /// ∩ [280, 282] = [11500/41, 282] ≈ [280.49, 282.00].
    #[test]
    fn test_e113_carlin_minimum_correction_set_and_interval() {
        let (cells, ids) = e113(&[]);
        let r = rules();
        let found = blame(&cells, &ids, &r).expect("blames");
        let b = the_blame(&found);
        let q1 = loc(Q1, "QTD", "B5");
        assert_eq!(
            conflict_cells(b),
            vec![
                vec![q1.clone(), loc(Q1, "QTD", "C5")],
                vec![q1.clone(), loc(Q2, "QTD", "B5"), loc(Q2, "YTD", "B5")],
            ]
        );
        for set in &b.conflicts {
            assert!(check_conflict_set(&cells, &ids, &r, set).is_ok(), "{set:?}");
        }
        assert_eq!(b.label, CorrectionLabel::Minimum);
        assert_eq!(b.corrections.len(), 1);
        assert_eq!(b.corrections[0].cells, vec![q1.clone()]);
        assert_eq!(
            b.corrections[0].implied,
            vec![(q1, Ok(closed(q(11_500, 41), int(282))))]
        );
    }

    /// E113's text says 281 satisfies both identities; 282 does too. The output is the range, never one fix.
    #[test]
    fn test_e113_both_281_and_282_lie_in_the_implied_interval() {
        let (cells, ids) = e113(&[]);
        let found = blame(&cells, &ids, &rules()).expect("blames");
        let b = the_blame(&found);
        let Ok(range) = &b.corrections[0].implied[0].1 else {
            panic!("no range: {b:?}");
        };
        assert_eq!(range.contains(int(281)), Ok(true));
        assert_eq!(range.contains(int(282)), Ok(true));
        assert_ne!(range.lo, range.hi);
        let q1 = loc(Q1, "QTD", "B5");
        for printed in [281, 282] {
            let (cells, ids) = e113(&[(q1.as_str(), printed)]);
            let components = solve(&cells, &ids, &rules()).expect("solves");
            assert!(
                matches!(components[0].outcome, Outcome::Feasible { .. }),
                "{printed}"
            );
        }
    }

    /// Negative control: with Q1 printed 281 the component is feasible, so there is no blame.
    #[test]
    fn test_blame_absent_when_feasible() {
        let (cells, ids) = e113(&[(loc(Q1, "QTD", "B5").as_str(), 281)]);
        assert!(blame(&cells, &ids, &rules()).expect("blames").is_empty());
    }

    /// E114 (Turquoise Ridge, 61.5%): Q1 148 + Q2 130 against H1 281, and H1 attributable C7 171. H1 alone is
    /// blamed, with range [170.5/0.615, 171.5/0.615] = [34100/123, 34300/123] ≈ [277.24, 278.86] (inside the
    /// half year's [277, 279]).
    #[test]
    fn test_e114_turquoise_ridge_h1_blamed() {
        let h1 = loc(Q2, "YTD", "B7");
        let ids = vec![
            half_year("tr_h1", &h1, &loc(Q1, "QTD", "B7"), &loc(Q2, "QTD", "B7")),
            share("tr_h1_share", &loc(Q2, "YTD", "C7"), &h1, q(123, 200)),
        ];
        let cells = cells("E114", &[]);
        let r = rules();
        let found = blame(&cells, &ids, &r).expect("blames");
        let b = the_blame(&found);
        assert_eq!(b.conflicts.len(), 2);
        for set in &b.conflicts {
            assert!(check_conflict_set(&cells, &ids, &r, set).is_ok(), "{set:?}");
        }
        assert_eq!(b.label, CorrectionLabel::Minimum);
        assert_eq!(b.corrections.len(), 1);
        assert_eq!(
            b.corrections[0].implied,
            vec![(h1, Ok(closed(q(34_100, 123), q(34_300, 123))))]
        );
    }

    /// E115 (Jabal Sayid, 50%, copper kt): Q1 18 + Q2 14 = 32 against H1 30, and half of Q2's 14 is 7 against
    /// the attributable 6. Two certified conflicts, both through Q2's 100% figure, whose range is then [11, 13].
    #[test]
    fn test_e115_jabal_sayid_two_certified_conflicts() {
        let q2 = loc(Q2, "QTD", "B28");
        let ids = vec![
            half_year("js_h1", &loc(Q2, "YTD", "B28"), &loc(Q1, "QTD", "B28"), &q2),
            share("js_q2_share", &loc(Q2, "QTD", "C28"), &q2, q(1, 2)),
        ];
        let cells = cells("E115", &[]);
        let r = rules();
        let found = blame(&cells, &ids, &r).expect("blames");
        let b = the_blame(&found);
        assert_eq!(
            conflict_cells(b),
            vec![
                vec![loc(Q1, "QTD", "B28"), q2.clone(), loc(Q2, "YTD", "B28")],
                vec![q2.clone(), loc(Q2, "QTD", "C28")],
            ]
        );
        for set in &b.conflicts {
            assert!(check_conflict_set(&cells, &ids, &r, set).is_ok(), "{set:?}");
        }
        assert_eq!(b.label, CorrectionLabel::Minimum);
        assert_eq!(b.corrections.len(), 1);
        assert_eq!(
            b.corrections[0].implied,
            vec![(q2, Ok(closed(int(11), int(13))))]
        );
    }

    /// E116 (Hemlo, sold 26 November 2025): H1 2026 cost of sales per ounce YTD!E11 prints 1691 while Minestats1's
    /// Hemlo H1 2026 cell N103 prints '—'. The conflict is located on E11, the side with a printed value; "equal
    /// to Pueblo Viejo's E10" is a signature labelled HYPOTHESIS, never the cause.
    #[test]
    fn test_e116_hemlo_presence_conflict_with_hypothesis_signature() {
        let cells = cells("E116", &[]);
        let placeholders: Vec<Placeholder> = PLACEHOLDERS
            .iter()
            .map(|&(_, file, sheet, a1, token)| Placeholder {
                locator: loc(file, sheet, a1),
                sha256: sha256(file).to_string(),
                token: token.to_string(),
            })
            .collect();
        let (e11, n103) = (loc(Q2, "YTD", "E11"), loc(Q2, "Minestats1", "N103"));
        let ids = vec![Identity {
            id: "hemlo_cos_per_oz_h1".to_string(),
            family: "same_figure".to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Equal {
                a: e11.clone(),
                b: n103.clone(),
            },
        }];
        let found = presence_conflicts(&cells, &placeholders, &ids).expect("no refusal");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].located, e11);
        assert_eq!(found[0].placeholder, n103);
        assert_eq!(found[0].signatures.len(), 1);
        assert_eq!(found[0].signatures[0].label, SignatureLabel::Hypothesis);
        assert_eq!(found[0].signatures[0].equal_to, loc(Q2, "YTD", "E10"));
        // The linear solver does not take a placeholder as a cell.
        assert!(matches!(
            solve(&cells, &ids, &rules()),
            Err(XfRefusal::UnknownCell { .. })
        ));
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// Every fixture cites a registered file, under the sha256 the registry holds.
    #[test]
    fn test_fixture_citations_match_the_registry() {
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        for (file, sha) in SHA256 {
            let registered = profile
                .sources
                .iter()
                .find(|s| s.file_name == file)
                .expect("registered");
            assert_eq!(registered.sha256_hex, sha, "{file}");
        }
        let cited: Vec<&str> = CELLS
            .iter()
            .map(|c| c.1)
            .chain(PLACEHOLDERS.iter().map(|p| p.1))
            .collect();
        assert!(cited.iter().all(|f| SHA256.iter().any(|(s, _)| s == f)));
    }

    /// Reader two: every fixture cell read back through `fmine-io` from the registered bytes (BD-339's job). One
    /// log line per cell and a summary; values are not logged (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_blame_fixtures_match_registered_bytes() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let sources = load_registered_sources(&profile, &root)
            .expect("registered files present and unaltered");
        let read = |file: &str, sheet: &str, a1: &str| -> CellValue {
            let src = sources
                .iter()
                .find(|s| s.file_name == file)
                .expect("loaded");
            let (row, col) = parse_a1_ref(a1).expect("an A1 reference");
            src.workbook
                .sheet(sheet)
                .and_then(|ws| ws.cell(row, col))
                .map(|c| c.value.clone())
                .expect("a stored cell")
        };
        let stored_text = |file: &str, sheet: &str, a1: &str| -> Option<String> {
            let src = sources.iter().find(|s| s.file_name == file)?;
            let (row, col) = parse_a1_ref(a1)?;
            src.workbook
                .sheet(sheet)?
                .cell(row, col)?
                .number_text
                .clone()
        };
        let (mut cells_checked, mut passed) = (0, 0);
        let mut fixtures: Vec<&str> = Vec::new();
        for &(evidence, file, sheet, a1, printed) in &CELLS {
            let pass = matches!(read(file, sheet, a1), CellValue::Number(_))
                && stored_text(file, sheet, a1).and_then(|t| Rat::from_decimal_str(&t).ok())
                    == Some(int(printed));
            println!(
                "{{\"fixture\":\"{evidence}\",\"locator\":\"{}\",\"sha256\":\"{}\",\"pass\":{pass}}}",
                loc(file, sheet, a1),
                sha256(file)
            );
            cells_checked += 1;
            passed += usize::from(pass);
            if !fixtures.contains(&evidence) {
                fixtures.push(evidence);
            }
        }
        for &(evidence, file, sheet, a1, token) in &PLACEHOLDERS {
            let pass = read(file, sheet, a1) == CellValue::Placeholder(token.to_string());
            println!(
                "{{\"fixture\":\"{evidence}\",\"locator\":\"{}\",\"sha256\":\"{}\",\"pass\":{pass}}}",
                loc(file, sheet, a1),
                sha256(file)
            );
            cells_checked += 1;
            passed += usize::from(pass);
        }
        println!(
            "{{\"fixtures\":{},\"cells\":{cells_checked},\"pass\":{passed}}}",
            fixtures.len()
        );
        assert_eq!(passed, cells_checked);
    }

    /// bd-4wvl.7 (BD-340.S7): the full power study (10³ injections per kind) and the kill report of plan K.5
    /// row 3. Every entry is computed here, none set by hand:
    /// - joint-only detections: the study's, by kind, and COD-02's shared total;
    /// - one-cell blame on E113 and E114 (S3's fixtures above);
    /// - discovery on E698 (S4's tie fixture, as the bead transcribes it) and E815 (S4 on the registered AISC
    ///   tables);
    /// - implied-interval consumers: the beads whose production source reads a correction set's implied ranges.
    ///
    /// E144 (S5's roll-forward) and E147 (S4 part 4's deck) have not landed and stay `null`. No cell value is
    /// logged (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_power_study_full_and_kill_report() {
        use fmine_numerics::exact_feas::discover::{
            Discovered, Printed, TieFinding, discover_tie_conventions,
        };
        use fmine_numerics::exact_feas::generator::{GenSpec, SUM_FAMILY};
        use fmine_numerics::exact_feas::power::{
            KillReport, PowerSpec, joint_detects, joint_only, power_study, size1_blame,
        };

        let start = std::time::Instant::now();
        let spec = PowerSpec {
            seed: 3407,
            per_kind: 1000,
            generator: GenSpec::all(),
            max_tables: 100_000,
        };
        let study = power_study(&spec, joint_detects).expect("the study runs");
        for line in study.log_lines() {
            println!("{line}");
        }
        for r in &study.rows {
            assert_eq!(r.injections(), 1000, "{:?}", r.injection);
        }

        let mut joint_only_detections = study.joint_only_by_kind();
        let cent = q(1, 100);
        let synthetic = |l: &str, printed: Rat, step: Rat| Cell {
            locator: l.to_string(),
            sha256: "0".repeat(64),
            token: format!("{}/{}", printed.num(), printed.den()),
            printed,
            step,
            scope: Scope::Visible,
        };
        let cod02 = vec![
            synthetic("T", int(10), int(1)),
            synthetic("a1", q(26, 5), cent),
            synthetic("a2", q(26, 5), cent),
            synthetic("b1", q(24, 5), cent),
            synthetic("b2", q(24, 5), cent),
        ];
        let footing = |id: &str, lines: [&str; 2]| Identity {
            id: id.to_string(),
            family: SUM_FAMILY.to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Sum {
                total: "T".to_string(),
                lines: lines.iter().map(|l| (*l).to_string()).collect(),
                signs: vec![1, 1],
            },
        };
        let cod02_ids = vec![footing("A", ["a1", "a2"]), footing("B", ["b1", "b2"])];
        let cod02_rules =
            Rules::new(Convention::Closed).with(SUM_FAMILY, Construction::RoundedOnce);
        let cod02_joint_only =
            joint_only(&cod02, &cod02_ids, &cod02_rules, joint_detects).expect("solves");
        joint_only_detections.push(("COD-02".to_string(), u64::from(cod02_joint_only)));

        let (e113_cells, e113_ids) = e113(&[]);
        let e113_size1 = size1_blame(&e113_cells, &e113_ids, &rules()).expect("blames");
        let h1 = loc(Q2, "YTD", "B7");
        let e114_ids = vec![
            half_year("tr_h1", &h1, &loc(Q1, "QTD", "B7"), &loc(Q2, "QTD", "B7")),
            share("tr_h1_share", &loc(Q2, "YTD", "C7"), &h1, q(123, 200)),
        ];
        let e114_size1 = size1_blame(&cells("E114", &[]), &e114_ids, &rules()).expect("blames");

        let printed = |document: &str, quantity: &str, v: Rat| Printed {
            document: document.to_string(),
            quantity: quantity.to_string(),
            printed: v,
            step: q(1, 10),
        };
        let e698 = discover_tie_conventions(&[
            printed("deck", "depletion", q(36, 10)),
            printed("deck", "net_change", q(17, 10)),
            printed("mda", "depletion", q(37, 10)),
            printed("mda", "net_change", q(18, 10)),
        ])
        .expect("discovery");
        let e698_found = e698
            .findings
            .iter()
            .any(|(_, f)| matches!(f, TieFinding::Identified(_)));
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let assurance = fmine_public::run_public_assurance(&profile, &root)
            .expect("registered files present and unaltered");
        let e815_found = assurance
            .construction_discovery()
            .expect("discovery")
            .iter()
            .any(|(_, d)| d.report.discovered != Discovered::Ambiguous);

        // A bead consumes implied intervals when its production source (before its test module) reads a
        // correction set's `.implied`.
        let consumers = [
            ("BD-331", "crates/fmine-public/src/mine_ledger.rs"),
            ("BD-349", "crates/fmine-cli/src/packet.rs"),
        ];
        let mut implied_consumers = Vec::new();
        for (bead, path) in consumers {
            let text = std::fs::read_to_string(root.join(path)).expect("readable source");
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            if production.contains(".implied") {
                implied_consumers.push(bead.to_string());
            }
        }

        let report = KillReport {
            joint_only_detections,
            size1: vec![
                ("E113".to_string(), Some(e113_size1)),
                ("E114".to_string(), Some(e114_size1)),
                ("E144".to_string(), None),
            ],
            discoveries: vec![
                ("E147".to_string(), None),
                ("E698".to_string(), Some(e698_found)),
                ("E815".to_string(), Some(e815_found)),
            ],
            implied_consumers,
        };
        println!(
            "{}",
            report.log_line(spec.seed, start.elapsed().as_secs_f64())
        );
        // The known answers S3 and S4 already establish.
        assert!(cod02_joint_only && e113_size1 && e114_size1 && e698_found && e815_found);
        assert_eq!(report.pending(), ["E144", "E147"]);
    }
}
