//! Real-workbook public-lane integration tests against the independent Python readers and pinned censuses.
//! DESIGN: cross-crate end-to-end checks live in the T layer; product APIs and oracle assertions are unchanged.

#[cfg(test)]
mod corpus {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};

    use fmine_core::hash::Digest32;
    use fmine_io::xlsx::{MinimalCell, Workbook};
    use fmine_numerics::exact_feas::discover::UnitFinding;
    use fmine_public::aisc::{
        self, Calc, ConsistentBasis, IdentityFamily, IdentityInstance, IdentityState,
        OneCellOutcome, OperandRole, OperandValue, RefusedReason, Undecided,
    };
    use fmine_public::sources::IssuerProfile;
    use fmine_public::{
        AlternateCounts, discovery_json, load_issuer_profile, mine_ledger, run_public_assurance,
        sources, synthetic,
    };

    const NO_FOOTNOTE: IdentityState = IdentityState::Refused {
        reason: RefusedReason::FootnoteNotFound,
    };

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn hex(bytes: &[u8; 32]) -> String {
        Digest32(*bytes).to_hex()
    }

    /// bd-jg43.1 parity: reading styles changes no census count and no numeric digest. The values were printed
    /// by this test at commit 345e2ba, before the styles reader existed, and pasted here. bd-jg43.3 styled the
    /// AISC workbook and added column N (11 numbers and the header "Site G"): its census must equal that of the
    /// same cells written as plain numbers with no styles part.
    #[test]
    fn test_census_unchanged_by_style_reading() {
        let line = |name: &str, bytes: &[u8]| {
            let c = Workbook::from_bytes(name, bytes)
                .expect("synthetic workbook")
                .cell_census();
            format!(
                "{name} {} {} {} {} {} {} {}",
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
                hex(&c.numeric_digest)
            )
        };
        let got: Vec<String> = synthetic::synthetic_workbooks(&[])
            .into_iter()
            .map(|(name, _, _, bytes)| line(name, &bytes))
            .collect();
        assert_eq!(
            got[0],
            "Synthetic_Mine_Stats.xlsx 2 0 2 0 0 9 ed492958121280b991212c61d01c9c2a257a551dfb07af145f1f6c117e646c21"
        );
        let plain: Vec<(String, MinimalCell)> = synthetic::synthetic_aisc_cells()
            .into_iter()
            .map(|(r, c)| match c {
                MinimalCell::Styled { raw, .. } => {
                    (r, MinimalCell::Number(raw.parse().expect("decimal")))
                }
                other => (r, other),
            })
            .collect();
        let unstyled = fmine_io::xlsx::build_minimal_xlsx(&[
            fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
                hidden: false,
                cells: plain,
            },
            fmine_io::xlsx::MinimalSheet {
                name: "Scratch".to_string(),
                hidden: true,
                cells: vec![("A1".to_string(), MinimalCell::Number(1.0))],
            },
            fmine_io::xlsx::MinimalSheet {
                name: synthetic::SYNTHETIC_HIDDEN_SHEET.to_string(),
                hidden: true,
                cells: synthetic::synthetic_hidden_cells(),
            },
        ]);
        assert_eq!(got[1], line(synthetic::SYNTHETIC_AISC_FILE, &unstyled));
        assert!(
            got[1].starts_with("Synthetic_AISC.xlsx 1 2 118 4 1 29 "),
            "{}",
            got[1]
        );
    }

    fn js(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    fn barrick_to_2026q2(root: &Path) -> IssuerProfile {
        load_issuer_profile(root, "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound")
    }

    /// GATE-BULL-01 on the real registered workbooks. Every expected value below comes from the independent
    /// Python reader written before this code (scratch `fidelity.py` and `census2.py`, 2026-10-29; recorded in
    /// MOR_CASE E820–E823), never from this implementation's own output.
    ///
    /// The run is bounded `--through 2026Q2`, so it keeps testing the same inputs after later vintages are
    /// registered. The state root at the end is different in kind: it is a regression pin of this
    /// implementation's own output (MOR_CASE E822, updated 2026-10-29 for outward rate rounding), not an
    /// independent value.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_real_files_match_the_independent_reader() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        assert!(r.gate_eligible());
        assert_eq!(r.through.as_deref(), Some("2026Q2"));

        // Part 1, extraction fidelity: visible, hidden, numeric, placeholder, error, text cells and the digest.
        let oracle: &[(&str, [usize; 6], &str)] = &[
            (
                "Barrick_Q1_2024_Mine_Stats.xlsx",
                [6, 0, 1130, 298, 0, 251],
                "293c9e29be4e09cc0eb380b2d80bcbca5aae5656bd65284f275b3265dafc2691",
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                [6, 0, 1141, 197, 0, 243],
                "f4636a96aa9f97c2e29969c5cd551b107e6a887129454074a527d27315b788a7",
            ),
            (
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                [6, 0, 1112, 242, 0, 249],
                "e96c6202baddf1fea717d8b8ae3fb55211f5a1fe4e35f152f4e8d92eca0dc7a3",
            ),
            (
                "Barrick_Q2_2024_Mine_Stats.xlsx",
                [7, 0, 2299, 556, 0, 490],
                "23e8da7796431d1a13cbbace051ffd7fb048f3dc1e96311b55e2c27853cf0b19",
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                [7, 0, 2304, 370, 0, 475],
                "e251c3f9b10fd4355e91d4518d02c94d27da097d80e6e3045f99bcb6a66845d0",
            ),
            (
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                [7, 0, 2297, 480, 1092, 494],
                "cab9164142f442cd240606ae7a0652c8953f449c9a4b98f2f8cf081e255ad3b2",
            ),
            (
                "Barrick_Q3_2024_Mine_Stats.xlsx",
                [7, 0, 2305, 550, 0, 489],
                "7b035d292541cd6693239816ea07ed9fb68930887052d8fcd5ffb3baedfc085c",
            ),
            (
                "Barrick_Q3_2025_Mine_Stats.xlsx",
                [7, 0, 2311, 363, 0, 475],
                "e18d37f3b0df75d238a227519e40ffac5e553b00684e489b135e834b11856ad3",
            ),
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                [7, 0, 2308, 546, 0, 489],
                "bbf38e19e04a4d3d1bfe9fe0b26afba69863065873e6459d716e67328038a3b1",
            ),
            (
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                [7, 0, 2357, 350, 0, 486],
                "ddb6af2ae1508e85d01ed3445fb3ce54883e3791a5510427b8c21cdcadef9efb",
            ),
            (
                "Barrick_Q1_2025_AISC.xlsx",
                [3, 0, 1146, 0, 0, 243],
                "fb649ed9d8e09f54f4569c482266e1f33c2017f2045d5d2f65671b5ba0112cfe",
            ),
            (
                "Barrick_Q1_2026_AISC.xlsx",
                [3, 2, 680, 0, 0, 154],
                "38dcba35d809105e525637133ca349722502e36d4facce667c2810d54d63e574",
            ),
            (
                "Barrick_Q2_2025_AISC.xlsx",
                [5, 0, 1910, 0, 0, 400],
                "fdabe9e9ca38b3caa89b14b769ea1f74777ca59220af04b2355d9ac7f6a66fc9",
            ),
            (
                "Barrick_Q2_2026_AISC.xlsx",
                [5, 2, 1088, 0, 0, 254],
                "dbef0e1bab051189f7fed9a7cd6e90ff8aff51ee3ad6a8d3f3181a6e53e59ffb",
            ),
            (
                "Barrick_Q3_2024_AISC.xlsx",
                [5, 0, 1910, 0, 0, 410],
                "9a7c8fe4b3298c63a4abfd7dc89ab87933df588aca80941f7930713b945c1076",
            ),
            (
                "Barrick_Q3_2025_AISC.xlsx",
                [5, 0, 1610, 0, 0, 395],
                "82d1d5c8aae8e9718da38ae52704a49a16dea07e227d70050bb862127c7c9849",
            ),
            (
                "Barrick_Q4_2024_AISC.xlsx",
                [4, 1, 1528, 0, 0, 328],
                "8431c88ef3a474f1c97c4a3485669292145e28f5a37536b56c0e7d79b9f9df0d",
            ),
            (
                "Barrick_Q4_2025_AISC.xlsx",
                [4, 1, 952, 0, 0, 212],
                "954f03ad421d6d20ca4c062e5b11b239ee3c46e97e5ceb4cb49dade616f71d1e",
            ),
        ];
        assert_eq!(
            r.sources.len(),
            oracle.len(),
            "every loaded file has an oracle row"
        );
        for &(file, counts, digest) in oracle {
            let s = r.sources.iter().find(|s| s.file_name == file).expect(file);
            let c = &s.census;
            let got = [
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
            ];
            assert_eq!(got, counts, "{file} census");
            assert_eq!(hex(&c.numeric_digest), digest, "{file} numeric digest");
        }

        // Part 2, the identity ledger.
        assert_eq!((r.aisc_tables, r.instances.len()), (34, 2595));
        assert!(r.table_refusals.is_empty() && r.unregistered_headers.is_empty());
        let counts = r.state_counts();
        let get = |f: IdentityFamily, c: &'static str| counts.get(&(f, c)).copied().unwrap_or(0);
        use IdentityFamily::{AiscFooting, AiscPerOunce, CosPerOunce, TccFooting, TccPerOunce};
        // bd-jg43.3 (`census2.py --displayed`, E838): on displayed values E823's three (v) per footing family
        // and per TCC and AISC rate are (i).
        assert_eq!(
            [
                get(TccFooting, "i"),
                get(TccFooting, "iii"),
                get(TccFooting, "v"),
                get(TccFooting, "np")
            ],
            [505, 2, 0, 12]
        );
        assert_eq!(
            [
                get(AiscFooting, "i"),
                get(AiscFooting, "iii"),
                get(AiscFooting, "v"),
                get(AiscFooting, "np")
            ],
            [505, 2, 0, 12]
        );
        assert_eq!(
            [
                get(CosPerOunce, "i"),
                get(CosPerOunce, "iv"),
                get(CosPerOunce, "v"),
                get(CosPerOunce, "np")
            ],
            // BD-331.S3 (census E826): of the 379 rates refused before for a non-zero non-controlling-interest
            // line, 263 fit the stated share (i) and 65 miss (iv); BD-331.S4 (region_census.py): of the 51
            // region instances left, 41 are not refuted by their members (i) and 10 are (iv).
            [111 + 263 + 41, 17 + 65 + 10, 379 - 328 - 51, 12]
        );
        assert_eq!(
            [
                get(TccPerOunce, "i"),
                get(TccPerOunce, "v"),
                get(TccPerOunce, "np")
            ],
            [507, 0, 12]
        );
        assert_eq!(
            [
                get(AiscPerOunce, "i"),
                get(AiscPerOunce, "ii"),
                get(AiscPerOunce, "v"),
                get(AiscPerOunce, "np")
            ],
            [504, 3, 0, 12]
        );
        assert_eq!(
            r.alternate_counts(),
            AlternateCounts {
                // `census2.py --displayed`: 1,014 and 1,000 (E838; 1,008 and 994 on stored values).
                footing_consistent: 1014,
                // A stated-share fit sums one line (k = 1), so the alternative construction accepts all 263. The
                // 41 regions not refuted by their members are not refuted with the region's ounces kept in their
                // own box either (BD-331.S4; the kernel's own result, no independent reader computes it).
                rate_consistent: 1000 + 263 + 41
            }
        );

        let mut iii: Vec<(String, String, String, i64, i64)> = r
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                IdentityState::IssuerDiscrepancy(c) => Some((
                    i.file.clone(),
                    i.sheet.clone(),
                    i.cell.clone(),
                    c.printed,
                    c.component_sum,
                )),
                _ => None,
            })
            .collect();
        iii.sort();
        let col_of = |cell: &str| {
            cell.trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        };
        let found: Vec<(String, String, String, i64, i64)> = iii
            .into_iter()
            .map(|(f, s, c, p, sum)| (f, s, col_of(&c), p, sum))
            .collect();
        let want = |f: &str, c: &str, p: i64, s: i64| {
            (f.to_string(), "Q2 2024".to_string(), c.to_string(), p, s)
        };
        assert_eq!(
            found,
            vec![
                want("Barrick_Q2_2025_AISC.xlsx", "L", 13, 12),
                want("Barrick_Q2_2025_AISC.xlsx", "O", 70, 72),
                want("Barrick_Q3_2024_AISC.xlsx", "L", 13, 12),
                want("Barrick_Q3_2024_AISC.xlsx", "O", 70, 72),
            ]
        );
        let ii: Vec<String> = r
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                IdentityState::Explained { component, .. } => Some(format!(
                    "{}|{}|{}|{}",
                    i.file,
                    i.sheet,
                    component.text(),
                    i.state.label()
                )),
                _ => None,
            })
            .collect();
        // BD-331.S2: no issuer note states E816's rule, so all three are reconstructed.
        assert_eq!(
            ii,
            vec![
                "Barrick_Q1_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10|\
                 explained_reconstructed",
                "Barrick_Q2_2025_AISC.xlsx|Q1 2025|Barrick_Q1_2025_Mine_Stats.xlsx:Minestats3!B32=10|\
                 explained_reconstructed",
                "Barrick_Q2_2025_AISC.xlsx|YTD Q2 2025|Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B74=13|\
                 explained_reconstructed",
            ]
        );
        let iv_subjects: Vec<&str> = r
            .instances
            .iter()
            .filter(|i| i.state.code() == "iv")
            .map(|i| i.subject.as_str())
            .collect();
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Veladero").count(), 12);
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Hemlo").count(), 3);
        assert_eq!(iv_subjects.iter().filter(|s| **s == "Kibali").count(), 2);

        // bd-sjrh.1: every instance cites operands. Each (iii)'s total and summands are among them, read
        // separately from the footing, and the summands add to its component sum. Failure messages and the log
        // name cells and counts only, never values (RISK-31).
        let mut by_role: BTreeMap<&str, usize> = BTreeMap::new();
        for i in &r.instances {
            let at = format!("{}:{}!{}", i.file, i.sheet, i.cell);
            assert!(!i.operands.is_empty(), "{at} has no operands");
            for o in &i.operands {
                *by_role.entry(o.role.as_str()).or_default() += 1;
            }
            if let IdentityState::IssuerDiscrepancy(c) = &i.state {
                let value = |cell: &str| {
                    i.operands
                        .iter()
                        .find(|o| o.file == i.file && o.sheet == i.sheet && o.cell == cell)
                        .map(|o| &o.value)
                };
                assert!(
                    value(&c.total_cell) == Some(&OperandValue::Int(c.printed)),
                    "{at}: total operand differs from the printed total"
                );
                let mut sum = 0;
                for line in &c.line_cells {
                    match value(line) {
                        Some(OperandValue::Int(x)) => sum += x,
                        Some(OperandValue::Token(_) | OperandValue::Blank) => {}
                        _ => panic!("{at}: summand {line} not cited as a whole number or token"),
                    }
                }
                assert!(
                    sum == c.component_sum,
                    "{at}: cited summands do not add to the component sum"
                );
            }
        }
        let roles: Vec<String> = by_role
            .iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect();
        println!(
            "{{\"test\":\"real_operands\",\"instances\":{},\"operands\":{},\"by_role\":{{{}}}}}",
            r.instances.len(),
            by_role.values().sum::<usize>(),
            roles.join(",")
        );

        // bd-sjrh.1 part 3, cited numbers: Kibali's 'Q2 2024'!O11 certificate prints 70 against a component sum
        // of 72 (E822), cited at the registered sha256 (registry, not this run). Messages name no values.
        let json = r.to_json();
        let kibali = "Barrick_Q3_2024_AISC.xlsx";
        let sha = &profile
            .sources
            .iter()
            .find(|s| s.file_name == kibali)
            .expect("registered")
            .sha256_hex;
        let at = format!("\"{kibali}:Q2 2024!O11\"");
        let printed = format!("\"printed\":{{\"v\":70,\"at\":{at},\"sha256\":\"{sha}\"}}");
        let sum = "\"component_sum\":{\"v\":72,\"rule\":\"aisc.footing.component_sum\"";
        let residual = "\"residual\":{\"v\":-2,\"rule\":\"aisc.footing.residual\"";
        let cert = json
            .split("\"certificate\":")
            .find(|c| c.starts_with(&format!("{{{printed}")))
            .is_some_and(|c| c.contains(sum) && c.contains(residual));
        assert!(cert, "Kibali O11 certificate is not the cited triple");
        let (_, records) = json.split_once("\"instances\":[").expect("instances array");
        let records = records.split_once("],\"one_cell_cos\"").expect("end").0;
        let n_records = records.matches("{\"at\":").count();
        let n_cited = records.matches("\"cited\":{\"operands\":[{").count();
        assert_eq!((n_records, n_cited), (r.instances.len(), r.instances.len()));
        println!(
            "{{\"test\":\"real_cited\",\"records\":{n_records},\"with_operands\":{n_cited},\"payload_bytes\":{}}}",
            json.len()
        );

        // Regression pin (not independent; see the doc comment): root, anchor, totals, registry epoch and digest
        // of digests equal the last row of repro/barrick/expected.toml, which records why each root moved.
        let expected = sources::load_expected_roots(&sources::expected_roots_path(
            &workspace_root(),
            "barrick",
        ))
        .expect("expected roots");
        let last = expected.last().expect("at least one row");
        if let Err(why) = r.check_expected(last) {
            panic!("{why}");
        }
    }

    /// bd-jg43.3 on the real files: E823's 12 Loulo-Gounkoto instances (column K of `Q1 2026` in the Q1 2026
    /// workbook and of `Q2 2026` and `Q1 2026` in the Q2 2026 workbook; TCC and AISC footings and rates) leave
    /// (v), and every instance's state equals `census2.py --displayed`, run here, except the cost-of-sales rates
    /// that census leaves to the stated-share rule (`share_census.py`, checked by its own test). One line per
    /// E823 instance and a summary (`--nocapture`); operands are logged by cell, never by value (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_e823_twelve_instances_leave_v() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &root).expect("registered files present");
        let script = "import json, sys\nsys.path.insert(0, 'scripts/oracle')\nimport census2\n\
                      _, res = census2.run('.', displayed=True)\n\
                      for x in res:\n    print(json.dumps([x[0], x[1], x[2], x[4], x[5], str(x[6])]))\n";
        let out = std::process::Command::new("python3")
            .args(["-I", "-c", script])
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "census2.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let family = |f: &str| match f {
            "TCC" => "tcc_footing",
            "AISC" => "aisc_footing",
            "COS/oz" => "cos_per_oz",
            "TCC/oz" => "tcc_per_oz",
            _ => "aisc_per_oz",
        };
        // (file, sheet, column, family) → (state, detail) as the census decides it.
        let mut census: BTreeMap<(String, String, String, String), (String, String)> =
            BTreeMap::new();
        let stdout = String::from_utf8(out.stdout).expect("utf-8 census output");
        for line in stdout.lines() {
            let f: Vec<String> = line
                .trim_matches(['[', ']'])
                .split("\", \"")
                .map(|s| s.trim_matches('"').to_string())
                .collect();
            assert_eq!(f.len(), 6, "{line}");
            let key = (
                f[0].clone(),
                f[1].clone(),
                f[2].clone(),
                family(&f[3]).to_string(),
            );
            census.insert(key, (f[4].clone(), f[5].clone()));
        }
        assert_eq!(census.len(), r.instances.len());
        let (mut compared, mut share_rule, mut differ) = (0, 0, Vec::new());
        for i in &r.instances {
            let key = (
                i.file.clone(),
                i.sheet.clone(),
                i.column.clone(),
                i.family.as_str().to_string(),
            );
            let Some((state, detail)) = census.get(&key) else {
                panic!("census has no {key:?}");
            };
            if i.family == IdentityFamily::CosPerOunce && detail == "attributable share needed" {
                share_rule += 1;
                continue;
            }
            compared += 1;
            if state != i.state.code() {
                differ.push(format!(
                    "{}:{}!{} {}",
                    i.file,
                    i.sheet,
                    i.cell,
                    i.family.as_str()
                ));
            }
        }
        let e823: Vec<&IdentityInstance> = r
            .instances
            .iter()
            .filter(|i| {
                i.column == "K"
                    && i.subject == "Loulo-Gounkoto"
                    && matches!(
                        (i.file.as_str(), i.sheet.as_str()),
                        ("Barrick_Q1_2026_AISC.xlsx", "Q1 2026")
                            | ("Barrick_Q2_2026_AISC.xlsx", "Q2 2026" | "Q1 2026")
                    )
                    && i.family != IdentityFamily::CosPerOunce
            })
            .collect();
        for i in &e823 {
            let cells: Vec<String> = i.operands.iter().map(|o| js(&o.cell)).collect();
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"family\":\"{}\",\"state_before\":\"v\",\
                 \"state_after\":\"{}\",\"displayed_operands\":[{}]}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                i.family.as_str(),
                i.state.code(),
                cells.join(",")
            );
        }
        let alt = r.alternate_counts();
        let n = |c: &str| r.count_state(c);
        println!(
            "{{\"instances\":{},\"i\":{},\"ii\":{},\"iii\":{},\"iv\":{},\"v\":{},\"np\":{},\
             \"alternate_footing\":{},\"alternate_rates\":{},\"state_root\":\"{}\",\"compared\":{compared},\
             \"share_rule\":{share_rule},\"oracle_equal\":{}}}",
            r.instances.len(),
            n("i"),
            n("ii"),
            n("iii"),
            n("iv"),
            n("v"),
            n("np"),
            alt.footing_consistent,
            alt.rate_consistent,
            r.ledger_anchor.state_root.to_hex(),
            differ.is_empty()
        );
        assert!(
            differ.is_empty(),
            "states differ from census2.py --displayed: {differ:?}"
        );
        assert_eq!((compared, share_rule), (2595 - 379, 379));
        assert_eq!(e823.len(), 12);
        assert!(e823.iter().all(|i| i.state.code() == "i"));
    }

    /// bd-jg43.1 on the real files: `Workbook::format_census` equals the independent reader
    /// (`python3 -I scripts/oracle/formats.py`, committed at 345e2ba before the Rust existed), run here on the
    /// same files. Lines are compared whole in the oracle's canonical JSON. The totals are also checked against
    /// the table written into the bead before either reader ran. One line per file, one per mismatch, then a
    /// summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_format_census_matches_independent_reader() {
        let root = workspace_root();
        let profile = barrick_to_2026q2(&root);
        let mut names: Vec<&str> = profile
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names.len(), 18);
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|n| root.join(&profile.corpus_dir).join(n))
            .collect();
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/formats.py"))
            .args(&paths)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "formats.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let oracle: Vec<&str> = stdout.lines().collect();
        assert_eq!(oracle.len(), 19, "18 file lines and a summary");

        let render = |formats: &BTreeMap<String, fmine_io::xlsx::FormatCount>| {
            let body: Vec<String> = formats
                .iter()
                .map(|(code, c)| format!("{}:[{},{}]", js(code), c.numeric, c.non_integer))
                .collect();
            format!("{{{}}}", body.join(","))
        };
        let mut total: BTreeMap<String, fmine_io::xlsx::FormatCount> = BTreeMap::new();
        let mut mismatches = 0;
        for ((name, path), want) in names.iter().zip(&paths).zip(&oracle) {
            let wb = Workbook::from_file(path).expect("registered workbook");
            let census = wb.format_census();
            for (code, c) in &census {
                let t = total.entry(code.clone()).or_default();
                t.numeric += c.numeric;
                t.non_integer += c.non_integer;
            }
            let got = format!(
                "{{\"file\":{},\"sha256\":\"{}\",\"formats\":{}}}",
                js(name),
                hex(&wb.sha256),
                render(&census)
            );
            let equal = got == *want;
            let open = got.strip_suffix('}').expect("rendered object");
            println!("{open},\"oracle_equal\":{equal}}}");
            if !equal {
                mismatches += 1;
                println!("{{\"file\":{},\"rust\":{got},\"oracle\":{want}}}", js(name));
            }
        }
        let numeric_total: usize = total.values().map(|c| c.numeric).sum();
        let non_integer_total: usize = total.values().map(|c| c.non_integer).sum();
        let summary = format!(
            "{{\"files\":18,\"formats\":{},\"numeric_total\":{numeric_total},\"non_integer_total\":{non_integer_total}}}",
            render(&total)
        );
        let pass = mismatches == 0 && summary == oracle[18];
        let open = summary.strip_suffix('}').expect("rendered object");
        println!("{open},\"pass\":{pass}}}");
        assert_eq!(
            mismatches, 0,
            "per-file format census differs from formats.py"
        );
        assert_eq!(summary, oracle[18]);
        // The bead's table (bd-jg43.1, written from a scratch read before either reader): 10 formats, 30,388
        // numeric cells (E820's total), 6,256 of them non-integer.
        assert_eq!(
            (total.len(), numeric_total, non_integer_total),
            (10, 30_388, 6_256)
        );
    }

    /// bd-jg43.2 on the real files: `Workbook::displayed_census` equals `python3 -I scripts/oracle/formats.py
    /// --displayed` (committed at 58d5c98 before the Rust interpreter) per file, compared as whole canonical
    /// lines: shown, token and refused counts and the displayed digest. E823's Loulo-Gounkoto cells are checked
    /// by value. One line per file, then a summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn test_displayed_digest_matches_independent_reader() {
        let root = workspace_root();
        let profile = barrick_to_2026q2(&root);
        let mut names: Vec<&str> = profile
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names.len(), 18);
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|n| root.join(&profile.corpus_dir).join(n))
            .collect();
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/formats.py"))
            .arg("--displayed")
            .args(&paths)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "formats.py --displayed failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let oracle: Vec<&str> = stdout.lines().collect();
        assert_eq!(oracle.len(), 19, "18 file lines and a summary");

        let (mut shown, mut token, mut refused, mut mismatches) = (0, 0, 0, 0);
        for ((name, path), want) in names.iter().zip(&paths).zip(&oracle) {
            let wb = Workbook::from_file(path).expect("registered workbook");
            let c = wb.displayed_census();
            shown += c.shown;
            token += c.token;
            refused += c.refused;
            let got = format!(
                "{{\"file\":{},\"sha256\":\"{}\",\"shown\":{},\"token\":{},\"refused\":{},\"displayed_digest\":\"{}\"}}",
                js(name),
                hex(&wb.sha256),
                c.shown,
                c.token,
                c.refused,
                hex(&c.digest)
            );
            let equal = got == *want;
            let open = got.strip_suffix('}').expect("rendered object");
            println!("{open},\"oracle\":{want},\"pass\":{equal}}}");
            mismatches += usize::from(!equal);
            assert!(
                c.refusals
                    .iter()
                    .all(|r| r.contains(": general_non_integer: "))
            );
        }
        let summary =
            format!("{{\"files\":18,\"shown\":{shown},\"token\":{token},\"refused\":{refused}}}");
        println!("{summary}");
        assert_eq!(
            mismatches, 0,
            "displayed census differs from formats.py --displayed"
        );
        assert_eq!(summary, oracle[18]);
        assert_eq!(shown + token + refused, 30_388, "E820's numeric total");

        // E823: Loulo-Gounkoto's unrounded column K (Other K8, TCC K10, AISC K17) displays as integers. Q1 2026
        // stores -37.899642999999998, 132.100357, 133.100357; Q2 2026 stores -7.822133, 164.17786699999999,
        // 194.17786699999999. Rounded by hand to 0 decimals.
        for (file, sheet, want) in [
            (
                "Barrick_Q1_2026_AISC.xlsx",
                "Q1 2026",
                ["-38", "132", "133"],
            ),
            ("Barrick_Q2_2026_AISC.xlsx", "Q2 2026", ["-8", "164", "194"]),
            (
                "Barrick_Q2_2026_AISC.xlsx",
                "Q1 2026",
                ["-38", "132", "133"],
            ),
        ] {
            let wb = Workbook::from_file(&root.join(&profile.corpus_dir).join(file))
                .expect("registered workbook");
            let sh = wb.sheet(sheet).expect("sheet");
            let text = |r: u32| {
                sh.cell(r, 11)
                    .and_then(|c| c.displayed())
                    .map(|d| d.map(|d| d.text()).unwrap_or_default())
            };
            // No values in the failure message (RISK-31: the BD-339 CI log carries no issuer cell values).
            assert!(
                [text(8), text(10), text(17)] == want.map(|w| Some(w.to_string())),
                "{file} {sheet}: K8, K10 or K17 does not display as expected"
            );
        }
    }

    /// BD-331.S2 on the real files: every (iv), visible or hidden, names a missing fact. One JSON line per (iv)
    /// (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_every_iv_names_missing_fact() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        let mut by_fact: BTreeMap<(String, String), usize> = BTreeMap::new();
        for i in r.instances.iter().chain(&r.hidden_scope.instances) {
            let IdentityState::Undecidable { missing_fact, .. } = &i.state else {
                continue;
            };
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"subject\":{},\"family\":{},\"missing_fact\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                js(&i.subject),
                js(i.family.as_str()),
                js(missing_fact)
            );
            assert!(!missing_fact.is_empty());
            *by_fact
                .entry((i.subject.clone(), missing_fact.clone()))
                .or_default() += 1;
        }
        let visible_iv = r.count_state("iv");
        // 92 since BD-331.S4: ten regions lie outside their members' weighted means (region_census.py).
        assert_eq!(visible_iv, 92);
        let velad = (
            "Veladero".to_string(),
            "the ounce series behind the per-ounce rows (E691, E836)".to_string(),
        );
        // E837 (bd-10om): one text per subject, no longer a shared period-wide one.
        let kibali = "the cost of sales or ounce basis of its fourth-quarter rate: the printed ounces fall \
                      below what the rate implies (E817, E836, E837)";
        let hemlo = "the ounces behind its fourth-quarter and full-year rates: all three per-ounce rates \
                     imply fewer ounces than the ounces sold printed (E817, E836, E837)";
        let visible = |subject: &str, fact: &str| {
            r.instances
                .iter()
                .filter(|i| {
                    i.subject == subject
                        && matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                            if missing_fact == fact)
                })
                .count()
        };
        assert_eq!(visible(&velad.0, &velad.1), 12);
        assert_eq!(visible("Kibali", kibali), 2);
        assert_eq!(visible("Hemlo", hemlo), 3);
        let share = r
            .instances
            .iter()
            .filter(|i| {
                matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                    if missing_fact == aisc::SHARE_MISSING_FACT)
            })
            .count();
        assert_eq!(share, 65);
        let region = r
            .instances
            .iter()
            .filter(|i| {
                matches!(&i.state, IdentityState::Undecidable { missing_fact, .. }
                    if missing_fact == aisc::REGION_MISSING_FACT)
            })
            .count();
        assert_eq!(region, 10);
        println!(
            "{{\"visible_iv\":{visible_iv},\"hidden_iv\":{},\"by_subject_and_fact\":{}}}",
            r.hidden_scope
                .instances
                .iter()
                .filter(|i| i.state.code() == "iv")
                .count(),
            by_fact.len()
        );
    }

    /// bd-4wvl.4 (E815, E848) on the real files: the AISC tables' footings are `ComponentSum`, visible and hidden
    /// scopes apart. One JSON line per family and scope, then each footing that does not foot, as a locator.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_construction_discovery_real_aisc_tables() {
        use fmine_numerics::exact_feas::discover::Discovered;
        let root = workspace_root();
        let profile = barrick_to_2026q2(&root);
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        // The lines the discovery counts are the ones the ledger summed.
        for i in r.instances.iter().chain(&r.hidden_scope.instances) {
            if let (Calc::Footing { component_sum, .. }, "i" | "iii") = (i.calc, i.state.code()) {
                let lines: i64 = i
                    .operands
                    .iter()
                    .filter_map(|o| match (o.role, &o.value) {
                        (OperandRole::Line, OperandValue::Int(x)) => Some(*x),
                        _ => None,
                    })
                    .sum();
                assert_eq!(lines, component_sum, "{}:{}!{}", i.file, i.sheet, i.cell);
            }
        }
        let found = r.construction_discovery().expect("discovery");
        for (scope, d) in &found {
            println!("{}", discovery_json(scope, d));
        }
        let mut not_footing = Vec::new();
        for (scope, instances) in [
            ("visible", &r.instances),
            ("hidden", &r.hidden_scope.instances),
        ] {
            for i in instances.iter().filter(|i| i.state.code() == "iii") {
                if let IdentityState::IssuerDiscrepancy(c) = &i.state {
                    let at = format!("{}:{}!{} {}", i.file, i.sheet, i.cell, i.subject);
                    println!(
                        "{{\"scope\":{},\"family\":{},\"not_footing\":{},\"sign_flip_witness\":{}}}",
                        js(scope),
                        js(i.family.as_str()),
                        js(&at),
                        c.sign_flip_witness
                            .as_deref()
                            .map_or_else(|| "null".to_string(), js)
                    );
                    not_footing.push((scope, at, c.sign_flip_witness.clone()));
                }
            }
        }
        // E815's three cells: Kibali and Porgera in both files that print Q2 2024, North America hidden only.
        let kibali = |f: &str| {
            (
                "visible",
                format!("Barrick_{f}_AISC.xlsx:Q2 2024!O11 Kibali"),
                Some("O7".to_string()),
            )
        };
        let porgera = |f: &str| {
            (
                "visible",
                format!("Barrick_{f}_AISC.xlsx:Q2 2024!L18 Porgera"),
                None,
            )
        };
        not_footing.sort();
        let mut want = vec![
            porgera("Q2_2025"),
            kibali("Q2_2025"),
            porgera("Q3_2024"),
            kibali("Q3_2024"),
            (
                "hidden",
                "Barrick_Q4_2024_AISC.xlsx:Q4 2023!J11 North America".to_string(),
                None,
            ),
        ];
        want.sort();
        assert_eq!(not_footing, want);
        // E838's census: footing i 1,010 and iii 4 visible; E828's hidden census: 170 scoreable, 169 consistent.
        let counts: Vec<(&str, &str, usize, usize, usize, usize)> = found
            .iter()
            .map(|(s, d)| {
                (
                    *s,
                    d.family.as_str(),
                    d.totals,
                    d.foot_exact,
                    d.report.n,
                    d.report.foot_exact,
                )
            })
            .collect();
        assert_eq!(
            counts,
            [
                ("visible", "tcc_footing", 507, 505, 371, 370),
                ("visible", "aisc_footing", 507, 505, 307, 306),
                ("hidden", "tcc_footing", 85, 84, 58, 57),
                ("hidden", "aisc_footing", 85, 85, 58, 58),
            ]
        );
        for (scope, d) in &found {
            assert_eq!(d.report.indeterminate, 0);
            assert_eq!(
                d.report.discovered,
                Discovered::ComponentSum,
                "{scope} {}",
                discovery_json(scope, d)
            );
        }
    }

    /// bd-4wvl.4 part 5 (E463, W1-DR-02) on the real files: the 2024 copper rows labelled "(millions of pounds)"
    /// are in kt, found from their site-sheet pairs (counts only).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_e463_copper_unit_real_files() {
        let root = workspace_root();
        let profile = barrick_to_2026q2(&root);
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        let u = r.copper_unit_discovery().expect("unit discovery");
        println!(
            "{{\"pairs\":{},\"fits\":{:?},\"finding\":{:?}}}",
            u.pairs, u.fits, u.finding
        );
        assert_eq!(
            u.pairs, 112,
            "the 2024 rows labelled in pounds, paired with the summary"
        );
        assert_eq!(u.fits, [("kt".to_string(), 112), ("Mlb".to_string(), 0)]);
        assert_eq!(u.finding, UnitFinding::Identified("kt".to_string()));
    }

    /// BD-331.S2 on the real files: the hidden scope. Every count is from the hidden census
    /// (`python3 -I scripts/oracle/hidden_census.py .`, which runs census2.py and share_census.py unchanged on
    /// the hidden sheets). One JSON line per hidden instance and per hidden sheet, then a summary (`--nocapture`).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn barrick_one_cell_matches_independent_reader() {
        let root = workspace_root();
        let profile = barrick_to_2026q2(&root);
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/oracle/one_cell_cos.py"))
            .arg(&root)
            .current_dir(&root)
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "one_cell_cos.py failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).expect("utf-8 oracle output");
        let mut lines: Vec<&str> = stdout.lines().collect();
        let oracle_summary = lines.pop().expect("summary line");
        let key = |line: &str| {
            let f: Vec<&str> = line.splitn(4, "\", \"").collect();
            f[..3].join("|")
        };
        let oracle: BTreeMap<String, &str> = lines.iter().map(|l| (key(l), *l)).collect();
        assert_eq!(oracle.len(), lines.len(), "one oracle line per column");

        // The K.4 items (Appendix K.4 "Still open"; E691, E817, E819), as one_cell_cos.py states them.
        let year = |sheet: &str| {
            sheet
                .split(|c: char| !c.is_ascii_alphanumeric())
                .find(|t| {
                    t.len() == 4 && t.starts_with("20") && t.bytes().all(|b| b.is_ascii_digit())
                })
                .map(str::to_string)
        };
        let k4_item = |c: &fmine_public::aisc::OneCellCheck| {
            let y = year(&c.sheet);
            match c.subject.as_str() {
                "Veladero" => matches!(y.as_deref(), Some("2024" | "2025")),
                "Hemlo" => matches!(c.sheet.as_str(), "Q4 2025" | "2025"),
                "North Mara" => matches!(c.sheet.as_str(), "Q2 2026" | "YTD Q2 2026"),
                "Latin America & Asia Pacific" => y.as_deref() == Some("2023"),
                _ => false,
            }
        };
        let text = |s: Option<&str>| s.map_or_else(|| "null".to_string(), |s| format!("\"{s}\""));
        let mut mismatches = Vec::new();
        for c in &r.one_cell {
            let (basis, reason, at_full, at_share, lo, hi) = match &c.outcome {
                OneCellOutcome::Refused {
                    reason,
                    at_full,
                    at_share,
                } => (None, Some(*reason), *at_full, *at_share, None, None),
                OneCellOutcome::Checked {
                    basis: (a, b),
                    at_full,
                    at_share,
                    implied_oz: (lo, hi),
                    ..
                } => (
                    Some(format!("{a}/{b}")),
                    None,
                    Some(*at_full),
                    *at_share,
                    Some(lo.as_str()),
                    Some(hi.as_str()),
                ),
            };
            let flag = |b: Option<bool>| b.map_or_else(|| "null".to_string(), |b| b.to_string());
            let got = format!(
                "{{\"file\": \"{}\", \"sheet\": \"{}\", \"column\": \"{}\", \"subject\": \"{}\", \"kind\": \"{}\", \
                 \"basis\": {}, \"state\": \"{}\", \"reason\": {}, \"at_full\": {}, \"at_share\": {}, \
                 \"implied_oz_lo\": {}, \"implied_oz_hi\": {}, \"k4_item\": {}}}",
                c.file,
                c.sheet,
                c.column,
                c.subject,
                if c.region { "region" } else { "site" },
                text(basis.as_deref()),
                c.state(),
                text(reason),
                flag(at_full),
                flag(at_share),
                text(lo),
                text(hi),
                k4_item(c)
            );
            // RISK-31: the lines carry implied ounces, so a mismatch is named by its key and field names only.
            let fields =
                |line: &str| -> Vec<String> { line.split(", \"").map(str::to_string).collect() };
            match oracle.get(&key(&got)) {
                Some(want) if *want == got => {}
                Some(want) => {
                    let differ: Vec<String> = fields(want)
                        .iter()
                        .zip(fields(&got))
                        .filter(|(w, g)| *w != g)
                        .map(|(w, _)| w.split("\":").next().unwrap_or("").to_string())
                        .collect();
                    mismatches.push(format!("{} differs in {differ:?}", key(&got)));
                }
                None => mismatches.push(format!("{} not in the oracle", key(&got))),
            }
        }
        assert_eq!(r.one_cell.len(), oracle.len(), "columns checked");
        assert!(
            mismatches.is_empty(),
            "differ from one_cell_cos.py: {mismatches:?}"
        );

        let v = fmine_public::aisc::one_cell_verdict(&r.one_cell, &k4_item);
        let count = |state: &str| r.one_cell.iter().filter(|c| c.state() == state).count();
        let pattern: Vec<String> = v
            .pattern
            .iter()
            .map(|((s, (a, b), other), n)| {
                format!(
                    "{{\"subject\": \"{s}\", \"basis\": \"{a}/{b}\", \"passes_at_other_basis\": {}, \"columns\": {n}}}",
                    other.map_or_else(|| "null".to_string(), |o| o.to_string())
                )
            })
            .collect();
        let got = format!(
            "{{\"columns\": {}, \"pass\": {}, \"fail\": {}, \"refused\": {}, \"k4_fail\": {}, \"other_fail\": {}, \
             \"verdict\": \"{}\", \"pattern\": [{}]",
            r.one_cell.len(),
            count("pass"),
            count("fail"),
            count("refused"),
            v.k4_fail,
            v.other_fail,
            v.verdict,
            pattern.join(", ")
        );
        println!("{got}}}");
        assert!(
            oracle_summary.starts_with(&got),
            "summary differs from one_cell_cos.py"
        );
        assert_eq!(
            (r.one_cell.len(), count("pass"), v.fail, count("refused")),
            (519, 374, 82, 63)
        );
        assert_eq!((v.k4_fail, v.other_fail, v.verdict), (17, 65, "basis rule"));
        // Every failing column is a (iv) cost-of-sales-per-ounce instance of the ledger, and the reverse, but for
        // the regions refuted by their members (BD-331.S4), which the one-cell check leaves unbased.
        let mut fails: Vec<String> = r
            .one_cell
            .iter()
            .filter(|c| c.state() == "fail")
            .map(|c| format!("{}:{}!{}", c.file, c.sheet, c.column))
            .collect();
        let mut undecidable: Vec<String> = r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce && i.state.code() == "iv")
            .filter(|i| i.state.reason_code() != Some("region_outside_members"))
            .map(|i| format!("{}:{}!{}", i.file, i.sheet, i.column))
            .collect();
        fails.sort_unstable();
        undecidable.sort_unstable();
        assert_eq!(fails, undecidable);
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_hidden_scope_matches_census() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .expect("registry")
            .through("2026Q2")
            .expect("bound");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        let hidden = &r.hidden_scope;
        let mut sheets: BTreeMap<(String, String), BTreeMap<&str, usize>> = BTreeMap::new();
        let (mut footing, mut per_oz) = (BTreeMap::new(), BTreeMap::new());
        for i in &hidden.instances {
            println!(
                "{{\"file\":{},\"sheet\":{},\"column\":{},\"subject\":{},\"family\":{},\"state\":{}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.column),
                js(&i.subject),
                js(i.family.as_str()),
                js(i.state.code())
            );
            *sheets
                .entry((i.file.clone(), i.sheet.clone()))
                .or_default()
                .entry(i.state.code())
                .or_default() += 1;
            let tally = match i.family {
                IdentityFamily::TccFooting | IdentityFamily::AiscFooting => &mut footing,
                _ => &mut per_oz,
            };
            *tally.entry(i.state.code()).or_insert(0_usize) += 1;
        }
        for ((file, sheet), by_state) in &sheets {
            let states: Vec<String> = by_state
                .iter()
                .map(|(k, v)| format!("{}:{v}", js(k)))
                .collect();
            println!(
                "{{\"file\":{},\"sheet\":{},\"instances\":{},\"by_state\":{{{}}}}}",
                js(file),
                js(sheet),
                by_state.values().sum::<usize>(),
                states.join(",")
            );
        }
        // The census's numbers (2026-10-30): 6 hidden sheets, 435 instances. Its three regional rates in (v) are
        // decided by their members since BD-331.S4: `region_census.py --hidden` finds two not refuted (i) and one
        // refuted (iv), compared one by one in `barrick_regions_match_region_census`.
        let census_footing = BTreeMap::from([("i", 169), ("iii", 1), ("np", 4)]);
        let census_per_oz = BTreeMap::from([("i", 235 + 2), ("iv", 17 + 1), ("np", 6)]);
        let agrees = sheets.len() == 6
            && hidden.instances.len() == 435
            && footing == census_footing
            && per_oz == census_per_oz;
        println!(
            "{{\"hidden_sheets\":{},\"instances\":{},\"iii\":{},\"census_agrees\":{agrees}}}",
            sheets.len(),
            hidden.instances.len(),
            footing.get("iii").copied().unwrap_or(0)
        );
        assert_eq!((hidden.aisc_tables, sheets.len()), (6, 6));
        assert_eq!(hidden.instances.len(), 435);
        assert_eq!(footing, census_footing);
        assert_eq!(per_oz, census_per_oz);
        assert!(hidden.unregistered_headers.is_empty() && hidden.table_refusals.is_empty());
        // E815, E821: North America Q4 2023 total cash costs, hidden in the Q4 2024 file, prints 541 against 540.
        let na = hidden
            .instances
            .iter()
            .find(|i| i.state.code() == "iii")
            .expect("one (iii)");
        assert_eq!(
            (na.file.as_str(), na.sheet.as_str(), na.column.as_str()),
            ("Barrick_Q4_2024_AISC.xlsx", "Q4 2023", "J")
        );
        assert_eq!(
            (na.subject.as_str(), na.family),
            ("North America", IdentityFamily::TccFooting)
        );
        match &na.state {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum), (541, 540))
            }
            other => panic!("{other:?}"),
        }
        // Never counted: the visible ledger is unchanged by the hidden scope.
        assert_eq!((r.aisc_tables, r.instances.len()), (34, 2595));
    }

    /// BD-331.S3 on the real files. Every count is from the independent census E826
    /// (`python3 -I scripts/oracle/share_census.py .`, written without reading this crate). Prints one JSON line
    /// per cost-of-sales rate the rule decides, then a summary line (`--nocapture` to see them).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_share_rule_matches_census() {
        let profile = load_issuer_profile(&workspace_root(), "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run_public_assurance(&profile, &workspace_root())
            .expect("registered files present and unaltered");
        // fit, miss, unconfirmed, regions_v, no_registered_share, no_footnote. Since BD-331.S4 the census's
        // regions_v are decided by their members, which `barrick_regions_match_region_census` checks one by one.
        let mut summary = [0_usize; 6];
        let mut by_subject: BTreeMap<String, [usize; 3]> = BTreeMap::new();
        for i in r
            .instances
            .iter()
            .filter(|i| i.family == IdentityFamily::CosPerOunce)
        {
            let slot = match &i.state {
                _ if region_record(i).is_some() => 3,
                IdentityState::Consistent {
                    basis: Some(ConsistentBasis::Share(_)),
                    ..
                } => 0,
                IdentityState::Undecidable {
                    why: Undecided::ShareOutside { .. },
                    ..
                } => 1,
                IdentityState::RefusedAt { .. } => 2,
                IdentityState::Refused {
                    reason: RefusedReason::RegionalNumerator,
                } => 3,
                IdentityState::Refused {
                    reason: RefusedReason::NoRegisteredShare,
                } => 4,
                s if *s == NO_FOOTNOTE => 5,
                _ => continue,
            };
            summary[slot] += 1;
            if slot < 3 {
                by_subject.entry(i.subject.clone()).or_default()[slot] += 1;
            }
            // Locators and codes only: the feasible share interval and the detail carry issuer values (RISK-31).
            println!(
                "{{\"file\":{},\"sheet\":{},\"cell\":{},\"subject\":{},\"state\":{},\"reason_code\":{},\
                 \"slot\":{slot}}}",
                js(&i.file),
                js(&i.sheet),
                js(&i.cell),
                js(&i.subject),
                js(i.state.code()),
                js(i.state.reason_code().unwrap_or("none"))
            );
        }
        let [fit, miss, unconfirmed, regions_v, no_share, no_footnote] = summary;
        let sites = fit + miss + unconfirmed + no_share + no_footnote;
        println!(
            "{{\"sites\":{sites},\"fit\":{fit},\"miss\":{miss},\"unconfirmed\":{unconfirmed},\
             \"regions_v\":{regions_v},\"no_registered_share\":{no_share},\"no_footnote\":{no_footnote}}}"
        );
        assert_eq!((sites, summary), (328, [263, 65, 0, 51, 0, 0]));
        let census: BTreeMap<String, [usize; 3]> = [
            ("Bulyanhulu", 31, 3),
            ("Carlin", 9, 25),
            ("Cortez", 31, 3),
            ("Long Canyon", 3, 0),
            ("Loulo-Gounkoto", 23, 0),
            ("Nevada Gold Mines", 11, 23),
            ("North Mara", 30, 4),
            ("Phoenix", 34, 0),
            ("Pueblo Viejo", 29, 5),
            ("Tongon", 30, 0),
            ("Turquoise Ridge", 32, 2),
        ]
        .into_iter()
        .map(|(s, fit, miss)| (s.to_string(), [fit, miss, 0]))
        .collect();
        assert_eq!(by_subject, census);
    }

    /// The region census's record of an instance the region rule decided (BD-331.S4), as `[file, sheet, cell,
    /// region, verdict, reason, at, members, absent, excluded]` in Python's `json.dumps` form with the lists
    /// sorted (their order carries nothing); `None` for any other instance. A refusal's lists are empty (the
    /// Rust state carries none, so the census's are not compared), and a footing refusal names no cell (the
    /// census names none; the Rust names the region's ounces).
    fn region_record(i: &IdentityInstance) -> Option<String> {
        let (verdict, reason, at, weighting) = match &i.state {
            IdentityState::Consistent {
                basis: Some(ConsistentBasis::Members(w) | ConsistentBasis::MembersExact(w)),
                ..
            } => ("not_refuted", None, None, Some(w)),
            IdentityState::Undecidable {
                why: Undecided::RegionOutsideMembers { weighting, .. },
                ..
            } => ("refuted", None, None, Some(weighting)),
            IdentityState::Refused {
                reason: RefusedReason::RegionNoMembers,
            } => ("refused", Some("no_members"), None, None),
            IdentityState::RefusedAt {
                reason: RefusedReason::RegionOuncesDoNotFoot,
                ..
            } => ("refused", Some("region_ounces_do_not_foot"), None, None),
            IdentityState::RefusedAt {
                reason: RefusedReason::RegionMemberOperand,
                at,
            } => ("refused", Some("member_operand"), Some(at.as_str()), None),
            _ => return None,
        };
        let list = |names: Vec<&String>| {
            let mut names: Vec<String> = names.into_iter().map(|n| js(n)).collect();
            names.sort_unstable();
            format!("[{}]", names.join(", "))
        };
        let used = weighting.map_or_else(Vec::new, |w| w.used.iter().map(|m| &m.subject).collect());
        let absent = weighting.map_or_else(Vec::new, |w| w.absent.iter().collect());
        let excluded = weighting.map_or_else(Vec::new, |w| w.excluded.iter().collect());
        let opt = |s: Option<&str>| s.map_or_else(|| "null".to_string(), js);
        Some(format!(
            "[{}, {}, {}, {}, {}, {}, {}, {}, {}, {}]",
            js(&i.file),
            js(&i.sheet),
            js(&i.cell),
            js(&i.subject),
            js(verdict),
            opt(reason),
            opt(at),
            list(used),
            list(absent),
            list(excluded)
        ))
    }

    /// BD-331.S4 on the real files: the region rule decides exactly the cost-of-sales rates the stated-share
    /// census leaves to member weighting, visible and hidden, and each one's verdict, reason, cell and member
    /// lists equal the independent region census (`python3 -I scripts/oracle/region_census.py .` and
    /// `--hidden`, 57a5aa5, written without reading this crate), run here. Locators and names only (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn barrick_regions_match_region_census() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        // The census's own output, each line re-printed with its lists sorted and a refusal's emptied, then
        // its summary line as printed.
        let script = "import json, subprocess, sys\n\
            out = subprocess.run([sys.executable, '-I', 'scripts/oracle/region_census.py', '.'] + sys.argv[1:], \
            capture_output=True, text=True, check=True).stdout.splitlines()\n\
            keys = ('file', 'sheet', 'cell', 'region', 'verdict', 'reason', 'at')\n\
            for line in out[:-1]:\n    x = json.loads(line)\n    \
            lists = [[] if x['verdict'] == 'refused' else sorted(x[k]) for k in ('members', 'absent', 'excluded')]\n    \
            print(json.dumps([x[k] for k in keys] + lists, ensure_ascii=False))\n\
            print(out[-1])\n";
        let scopes: [(&str, &[IdentityInstance], &[&str], &str); 2] = [
            (
                "visible",
                &r.instances,
                &[],
                r#"{"regions": 51, "refuted": 10, "not_refuted": 41, "refused": 0}"#,
            ),
            (
                "hidden",
                &r.hidden_scope.instances,
                &["--hidden"],
                r#"{"regions": 3, "refuted": 1, "not_refuted": 2, "refused": 0}"#,
            ),
        ];
        for (scope, instances, flags, summary) in scopes {
            let out = std::process::Command::new("python3")
                .args(["-I", "-c", script])
                .args(flags)
                .current_dir(&root)
                .output()
                .expect("python3 runs");
            assert!(
                out.status.success(),
                "region_census.py failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).expect("utf-8 census output");
            let mut census: Vec<&str> = stdout.lines().collect();
            let census_summary = census.pop().expect("summary line");
            census.sort_unstable();
            let mut ours: Vec<String> = instances.iter().filter_map(region_record).collect();
            ours.sort_unstable();
            for line in &ours {
                println!("{{\"scope\":{},\"record\":{line}}}", js(scope));
            }
            println!("{{\"scope\":{},\"census\":{census_summary}}}", js(scope));
            assert_eq!(census_summary, summary, "{scope}");
            assert_eq!(ours, census, "{scope}");
        }
        // The census's check is exact (vertex enumeration), and so is the ledger's since E845: every region the
        // census does not refute is (i) with a checked witness, none `relaxation_only`.
        let exact = |instances: &[IdentityInstance]| {
            instances
                .iter()
                .filter(|i| {
                    matches!(
                        i.state,
                        IdentityState::Consistent {
                            basis: Some(ConsistentBasis::MembersExact(_)),
                            ..
                        }
                    )
                })
                .count()
        };
        assert_eq!(
            (exact(&r.instances), exact(&r.hidden_scope.instances)),
            (41, 2)
        );
        assert_eq!(r.relaxation_only(), 0);
    }

    /// A share as the census prints it: the percentage as an exact decimal (`61.5`, `84`).
    fn percent_text(s: fmine_numerics::exact_feas::Rat) -> String {
        use fmine_numerics::exact_feas::Rat;
        let mut p = s.checked_mul(Rat::from_integer(100)).expect("percent");
        let mut places = 0;
        while p.den() != 1 {
            p = p.checked_mul(Rat::from_integer(10)).expect("decimal");
            places += 1;
            assert!(places < 8, "share {s:?} is not a short decimal");
        }
        let digits = format!("{:0>width$}", p.num(), width = places + 1);
        let (int, frac) = digits.split_at(digits.len() - places);
        if places == 0 {
            int.to_string()
        } else {
            format!("{int}.{frac}")
        }
    }

    /// One Mine Stats ledger instance in `mine_stats_census.py`'s line format. The census names the withheld
    /// exclusion by its evidence (`excluded_e818`) and writes cumulative periods with a space (`2026 H1`).
    fn mine_census_line(l: &mine_ledger::MineLedger, i: &mine_ledger::Instance) -> String {
        use fmine_public::key::PeriodKind;
        use mine_ledger::{Reason, State};
        let opt = |s: Option<String>| s.map_or_else(|| "null".to_string(), |s| js(&s));
        let period = match i.period.kind {
            PeriodKind::Quarter(q) => format!("{}Q{q}", i.period.year),
            PeriodKind::H1 => format!("{} H1", i.period.year),
            PeriodKind::NineMonths => format!("{} 9M", i.period.year),
            PeriodKind::FullYear => format!("{} FY", i.period.year),
        };
        let (reason, at) = match &i.state {
            State::Refused { reason, at } => (
                Some(match reason {
                    Reason::Withheld { evidence } => {
                        format!("excluded_{}", evidence.to_lowercase())
                    }
                    other => other.code().to_string(),
                }),
                at.clone(),
            ),
            State::Unresolved(need) => (Some(need.code().to_string()), None),
            _ => (None, None),
        };
        let cells: Vec<String> = i.cells.iter().map(|c| opt(c.clone())).collect();
        let corrections = i.component.as_ref().map(|id| {
            let c = l
                .infeasible
                .iter()
                .find(|c| &c.id == id)
                .expect("component");
            let sets: Vec<String> = c
                .corrections
                .iter()
                .map(|s| {
                    format!(
                        "[{}]",
                        s.cells.iter().map(|x| js(x)).collect::<Vec<_>>().join(",")
                    )
                })
                .collect();
            format!("[{}]", sets.join(","))
        });
        format!(
            "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"anchor\":{},\"cells\":[{}],\
             \"state\":{},\"reason\":{},\"at\":{},\"certificate\":{},\"share\":{},\"printed_share\":{},\
             \"component\":{},\"corrections\":{}}}",
            js(i.family.as_str()),
            js(&i.subject),
            js(&i.field),
            js(i.group.as_str()),
            js(&period),
            js(&i.anchor),
            cells.join(","),
            js(i.state.code()),
            opt(reason),
            opt(at),
            opt(i.nil_token.then(|| "nil_token".to_string())),
            opt(i.share.map(percent_text)),
            opt(i.printed_share.clone()),
            opt(i.component.clone()),
            corrections.unwrap_or_else(|| "null".to_string())
        )
    }

    /// BD-331.S4 (bd-prmn.4): the Mine Stats ledger's ownership and quarter-sum instances equal the independent
    /// census (`scripts/oracle/mine_stats_census.py`, e66f7dc, written from the rule without reading this crate)
    /// line by line: states, reasons, operands, shares, components and every minimum correction set. Then the
    /// implied ranges of every set, compared in memory and named by component only (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/ and python3"]
    fn barrick_mine_ledger_matches_census() {
        use mine_ledger::Family;
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r =
            run_public_assurance(&profile, &root).expect("registered files present and unaltered");
        let l = &r.mine_ledger;
        let script = "import json, subprocess, sys\n\
            values = sys.argv[1:] == ['--values']\n\
            out = subprocess.run([sys.executable, '-I', 'scripts/oracle/mine_stats_census.py'] + sys.argv[1:] + \
            ['corpus_of_input_docs/downloaded/mine_stats'], capture_output=True, text=True, check=True).stdout\n\
            if not values:\n    print(out, end='')\n\
            else:\n    for line in out.splitlines():\n        x = json.loads(line)\n        \
            if 'joint_only' not in x or 'cells' not in x:\n            continue\n        \
            for k, d in enumerate(x['implied']):\n            for loc in sorted(d):\n                \
            lo, hi = d[loc]\n                \
            print('\\t'.join([x['component'], str(k), loc, 'unbounded' if 'unbounded' in (lo, hi) else lo + ',' + hi]))\n";
        let census = |flags: &[&str]| {
            let out = std::process::Command::new("python3")
                .args(["-I", "-c", script])
                .args(flags)
                .current_dir(&root)
                .output()
                .expect("python3 runs");
            assert!(
                out.status.success(),
                "mine_stats_census.py failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).expect("utf-8 census output")
        };
        let stdout = census(&[]);
        let lines: Vec<&str> = stdout.lines().collect();
        let instances: Vec<&str> = lines
            .iter()
            .copied()
            // An instance line is a family record with an anchor (E817's check line lists anchors too).
            .filter(|x| x.starts_with("{\"family\":") && x.contains("\"anchor\":"))
            .collect();
        let ours: Vec<String> = l.instances.iter().map(|i| mine_census_line(l, i)).collect();
        assert_eq!(ours.len(), instances.len(), "instance count");
        // A sum's operand order is a representation choice: the census lists members in its rule's map order,
        // the ledger in registry row order (they differ only for North America from 2025Q4). So a sum's member
        // cells are compared as a set, the aggregate's cell last. No sum here has an `at`, which order would set.
        let sum_cells_sorted = |line: &str| -> String {
            let sum = line.contains("\"family\":\"sites_to_region\"")
                || line.contains("\"family\":\"region_to_total\"");
            let Some(start) = line.find("\"cells\":[").map(|k| k + "\"cells\":[".len()) else {
                return line.to_string();
            };
            let Some(len) = line[start..].find(']') else {
                return line.to_string();
            };
            if !sum {
                return line.to_string();
            }
            let mut cells: Vec<&str> = line[start..start + len].split(',').collect();
            let aggregate = cells.pop();
            cells.sort_unstable();
            cells.extend(aggregate);
            format!(
                "{}{}{}",
                &line[..start],
                cells.join(","),
                &line[start + len..]
            )
        };
        // One log line per instance (the census's format: anchor, cells, family, state, reason, component,
        // implied corrections), printed before its comparison so a failing run shows every line up to the miss.
        for (a, b) in ours.iter().zip(&instances) {
            println!("{a}");
            assert_eq!(sum_cells_sorted(a), sum_cells_sorted(b));
        }
        // The sum families' counts (c697b13's census), pinned so that both sides cannot drift together.
        let counts = |f: Family| ["i", "iii", "v", "np"].map(|code| l.count(f, code));
        assert_eq!(counts(Family::SitesToRegion), [282, 0, 24, 0]);
        assert_eq!(counts(Family::RegionToTotal), [204, 0, 0, 0]);
        // Family 5 (ac07fee's census): i, iii, iv, v, np.
        let rate =
            ["i", "iii", "iv", "v", "np"].map(|code| l.count(Family::SoldWeightedRate, code));
        assert_eq!(rate, [252, 0, 30, 24, 0]);
        // Family 2 on the site sheets (9854b6c's census).
        assert_eq!(counts(Family::SiteQuarterSum), [2742, 8, 87, 683]);
        let components: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|x| x.contains("\"joint_only\":") && x.contains("\"instances\":"))
            .collect();
        let ours: Vec<String> = l
            .infeasible
            .iter()
            .map(|c| {
                let list = |xs: &[String]| xs.iter().map(|x| js(x)).collect::<Vec<_>>().join(",");
                let anchors: Vec<String> = c.instances.iter().map(|&k| l.instances[k].anchor.clone()).collect();
                let sets: Vec<String> = c.corrections.iter().map(|s| format!("[{}]", list(&s.cells))).collect();
                format!(
                    "{{\"component\":{},\"cells\":[{}],\"instances\":[{}],\"joint_only\":{},\"corrections\":[{}]}}",
                    js(&c.id),
                    list(&c.cells),
                    list(&anchors),
                    c.joint_only,
                    sets.join(",")
                )
            })
            .collect();
        assert_eq!(ours, components);
        assert!(
            l.infeasible.iter().all(|c| c.minimum),
            "every correction search exhaustive"
        );
        assert!(l.indeterminate.is_empty(), "{:?}", l.indeterminate);
        // The summary lines, as the census prints them.
        for family in Family::ALL {
            let mut by_reason: BTreeMap<String, usize> = BTreeMap::new();
            for i in l.instances.iter().filter(|i| i.family == family) {
                if let mine_ledger::State::Refused { reason, .. } = &i.state {
                    let code = match reason {
                        mine_ledger::Reason::Withheld { evidence } => {
                            format!("excluded_{}", evidence.to_lowercase())
                        }
                        other => other.code().to_string(),
                    };
                    *by_reason.entry(code).or_insert(0) += 1;
                }
            }
            let reasons: Vec<String> = by_reason
                .iter()
                .map(|(r, k)| format!("{}:{k}", js(r)))
                .collect();
            // Only family 5 can be (iv), and only its line counts it.
            let iv = if family == Family::SoldWeightedRate {
                format!(",\"iv\":{}", l.count(family, "iv"))
            } else {
                String::new()
            };
            let line = format!(
                "{{\"family\":{},\"n\":{},\"i\":{},\"iii\":{}{iv},\"v\":{},\"np\":{},\"v_by_reason\":{{{}}}}}",
                js(family.as_str()),
                l.instances.iter().filter(|i| i.family == family).count(),
                l.count(family, "i"),
                l.count(family, "iii"),
                l.count(family, "v"),
                l.count(family, "np"),
                reasons.join(",")
            );
            println!("{line}");
            assert!(lines.contains(&line.as_str()), "{line}");
        }
        let summary = format!(
            "{{\"components\":{},\"infeasible_components\":{},\"joint_only\":{}}}",
            l.components,
            l.infeasible.len(),
            l.infeasible.iter().filter(|c| c.joint_only).count()
        );
        println!("{summary}");
        assert!(lines.contains(&summary.as_str()), "{summary}");
        assert_eq!(
            summary,
            r#"{"components":2091,"infeasible_components":27,"joint_only":2}"#
        );

        // Implied ranges: one line per (component, set, cell), never printed.
        let census_ranges: Vec<String> =
            census(&["--values"]).lines().map(str::to_string).collect();
        let mut ours = Vec::new();
        for c in &l.infeasible {
            for (k, s) in c.corrections.iter().enumerate() {
                for (cell, range) in &s.implied {
                    let text = match range {
                        Ok(i) => {
                            assert!(
                                !i.lo_strict && !i.hi_strict,
                                "{}: closed boxes give closed ends",
                                c.id
                            );
                            format!(
                                "{},{}",
                                mine_ledger::rat_text(i.lo),
                                mine_ledger::rat_text(i.hi)
                            )
                        }
                        Err(why) => why.clone(),
                    };
                    ours.push(format!("{}\t{k}\t{cell}\t{text}", c.id));
                }
            }
        }
        assert!(!ours.is_empty(), "implied ranges compared");
        assert_eq!(ours.len(), census_ranges.len(), "implied range count");
        let differ: Vec<&str> = ours
            .iter()
            .zip(&census_ranges)
            .filter(|(a, b)| a != b)
            .filter_map(|(a, _)| a.split('\t').next())
            .collect();
        assert!(
            differ.is_empty(),
            "implied ranges differ in components {differ:?}"
        );

        // The plan's known answers (C1 DR-01, DR-02, DR-04): each a pair of (iii) on one shared cell.
        let state = |family: Family, anchor: &str| {
            l.instances
                .iter()
                .find(|i| i.family == family && i.anchor == anchor)
                .unwrap_or_else(|| panic!("no {} instance at {anchor}", family.as_str()))
        };
        for (e, own, sum, set) in [
            ("E113", "2026Q1:QTD!C5", "2026Q2:YTD!B5", "2026Q1:QTD!B5"),
            ("E114", "2026Q2:YTD!C7", "2026Q2:YTD!B7", "2026Q2:YTD!B7"),
            ("E115", "2026Q2:QTD!C28", "2026Q2:YTD!B28", "2026Q2:QTD!B28"),
        ] {
            let (o, s) = (
                state(Family::Ownership, own),
                state(Family::QuarterSum, sum),
            );
            assert_eq!((o.state.code(), s.state.code()), ("iii", "iii"), "{e}");
            assert_eq!(o.component, s.component, "{e}: one component");
            let c = l
                .infeasible
                .iter()
                .find(|c| Some(&c.id) == o.component.as_ref())
                .expect("component");
            let sets: Vec<&[String]> = c.corrections.iter().map(|s| s.cells.as_slice()).collect();
            assert_eq!(
                sets,
                [[set.to_string()].as_slice()],
                "{e}: the one minimum set"
            );
        }
        // E113: the range B5 can take from the other cells holds 281; the ledger never prints 281 as a value.
        let b5 = l
            .infeasible
            .iter()
            .flat_map(|c| &c.corrections)
            .flat_map(|s| &s.implied)
            .find(|(cell, _)| cell == "2026Q1:QTD!B5")
            .and_then(|(_, r)| r.as_ref().ok())
            .expect("B5's implied range");
        assert!(
            b5.contains(fmine_numerics::exact_feas::Rat::from_integer(281))
                .expect("exact")
        );
        // E116: the summary prints a number where the site sheet prints the nil token; decided as number = 0,
        // alone.
        let e116: Vec<&mine_ledger::Instance> = l
            .instances
            .iter()
            .filter(|i| i.family == Family::SiteSheet && i.anchor == "2026Q2:YTD!E11")
            .collect();
        assert_eq!(e116.len(), 1, "E116: one site_sheet instance");
        let e116 = e116.first().expect("E116");
        assert_eq!(e116.state.code(), "iii", "E116");
        assert!(e116.nil_token && e116.component.is_none(), "E116");
        assert_eq!(
            e116.cells,
            [
                Some("2026Q2:YTD!E11".to_string()),
                Some("2026Q2:Minestats1!N103".to_string())
            ],
            "E116"
        );
    }
}
