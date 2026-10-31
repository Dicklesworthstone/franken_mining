//! Known answers for bd-504e.1. Invented workbook values exercise actual extraction, identity assessment,
//! semantic keying and cross-vintage comparisons; the optional corpus test uses the registered public bytes.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx, col_index_to_letters,
    };
    use fmine_public::aisc::{AiscClaimIndex, IdentityFamily, key_aisc_operands};
    use fmine_public::consistency::{
        AiscConsistency, TransitionKind, aisc_consistency, clopper_pearson_95,
    };
    use fmine_public::key::parse_sheet_period;
    use fmine_public::sources::{SourceKind, load_issuer_profile};
    use fmine_public::{
        Lane, LoadedSource, PublicAssuranceReport, assess, load_registered_sources, synthetic,
    };

    fn source(
        file: &str,
        release: &str,
        sheet: &str,
        shift: u32,
        tcc: i64,
        hidden: bool,
    ) -> LoadedSource {
        let column = col_index_to_letters(2 + shift);
        let mut cells = vec![(
            format!("{column}2"),
            MinimalCell::Text("Site A".to_string()),
        )];
        for (row, label, value) in [
            (5, "Cost of sales applicable to gold production", 100),
            (6, "Royalty adjustment", 20),
            (7, "Total cash costs", tcc),
            (8, "Minesite sustaining", 30),
            (9, "All-in sustaining costs", 150),
            (10, "Ounces sold", 10),
            (11, "Cost of sales per ounce", 10_000),
            (12, "Total cash costs per ounce", 12_000),
            (13, "All-in sustaining costs per ounce", 15_000),
        ] {
            cells.push((format!("A{row}"), MinimalCell::Text(label.to_string())));
            cells.push((format!("{column}{row}"), MinimalCell::Number(value as f64)));
        }
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: sheet.to_string(),
            hidden,
            cells,
        }]);
        let workbook = Workbook::from_bytes(file.to_string(), &bytes).expect("synthetic xlsx");
        LoadedSource {
            file_name: file.to_string(),
            kind: SourceKind::AiscReconciliation,
            quarter: release.to_string(),
            url: "synthetic".to_string(),
            url_basis: "synthetic".to_string(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        }
    }

    fn run(sources: Vec<LoadedSource>) -> (PublicAssuranceReport, AiscClaimIndex, AiscConsistency) {
        let mut profile = synthetic::synthetic_profile();
        profile.subjects = vec!["Site A".to_string()];
        profile.reconstructions.clear();
        let report = assess(&profile, Lane::Synthetic, sources);
        let index = key_aisc_operands(&profile, &report.sources, &report.instances);
        assert!(index.refusals.is_empty(), "{:?}", index.refusals);
        let consistency = aisc_consistency(&index, &report.instances);
        (report, index, consistency)
    }

    fn pair(first: i64, later: i64) -> (PublicAssuranceReport, AiscClaimIndex, AiscConsistency) {
        run(vec![
            source("early.xlsx", "2024Q3", "YTD Sept 30 2024", 0, first, false),
            source("later.xlsx", "2025Q3", "YTD Q3 2024", 2, later, false),
        ])
    }

    #[test]
    fn test_period_normalisation_ytd_sept_30_equals_ytd_q3() {
        assert_eq!(
            parse_sheet_period("YTD Sept 30 2024"),
            parse_sheet_period("YTD Q3 2024")
        );
    }

    #[test]
    fn test_period_normalisation_ytd_dec_31_equals_fy() {
        assert_eq!(
            parse_sheet_period("YTD Dec 31 2024"),
            parse_sheet_period("2024")
        );
    }

    #[test]
    fn test_first_registered_dedup_counts_each_key_once() {
        let (report, _, consistency) = pair(120, 120);
        assert_eq!(report.instances.len(), 10);
        assert!(consistency.unkeyed.is_empty());
        assert_eq!(consistency.histories.len(), 5);
        assert_eq!(consistency.first_registered.values().sum::<usize>(), 5);
        assert_eq!(consistency.all_files, report.state_counts());
        for family in IdentityFamily::ALL {
            assert_eq!(consistency.first_registered[&(family, "i")], 1);
            assert_eq!(consistency.all_files[&(family, "i")], 2);
        }
        for history in &consistency.histories {
            assert_eq!(history.sequence.len(), 2);
            assert_eq!(history.transitions.len(), 1);
            assert_eq!(history.transitions[0].kind, TransitionKind::Persisted);
            assert!(history.sequence[0].at.contains("!B"));
            assert!(history.sequence[1].at.contains("!D"));
            assert_ne!(history.sequence[0].sha256, history.sequence[1].sha256);
        }
    }

    #[test]
    fn test_3a_uses_first_registered_not_latest() {
        let (_, _, consistency) = pair(119, 120);
        assert_eq!(
            consistency.first_registered[&(IdentityFamily::TccFooting, "iii")],
            1
        );
        assert!(
            !consistency
                .first_registered
                .contains_key(&(IdentityFamily::TccFooting, "i"))
        );
        assert_eq!(consistency.all_files[&(IdentityFamily::TccFooting, "i")], 1);
        let history = consistency
            .histories
            .iter()
            .find(|h| h.family == IdentityFamily::TccFooting)
            .unwrap();
        assert_eq!(history.sequence[0].release, "2024Q3");
        assert_eq!(history.transitions[0].kind, TransitionKind::Corrected);
    }

    #[test]
    fn test_dedup_ignores_hidden_sheets() {
        let (report, _, consistency) = run(vec![
            source("hidden.xlsx", "2024Q2", "Q2 2024", 0, 119, true),
            source("visible.xlsx", "2024Q3", "Q2 2024", 1, 120, false),
        ]);
        assert_eq!(report.hidden_scope.instances.len(), 5);
        assert_eq!(consistency.first_registered.values().sum::<usize>(), 5);
        assert_eq!(consistency.all_files.values().sum::<usize>(), 5);
        assert!(
            consistency
                .histories
                .iter()
                .all(|h| h.sequence[0].release == "2024Q3")
        );
        assert!(
            consistency
                .histories
                .iter()
                .all(|h| h.transitions[0].kind == TransitionKind::SingleVintage)
        );
    }

    #[test]
    fn test_3c_persisted_corrected_broken_reclassified_synthetic() {
        for (first, later, kind) in [
            (120, 120, TransitionKind::Persisted),
            (119, 120, TransitionKind::Corrected),
            (120, 119, TransitionKind::Broken),
            (119, 118, TransitionKind::Reclassified),
        ] {
            let (_, _, report) = pair(first, later);
            let h = report
                .histories
                .iter()
                .find(|h| h.family == IdentityFamily::TccFooting)
                .unwrap();
            assert_eq!(h.transitions[0].kind, kind, "{first} -> {later}");
            assert_eq!(h.transitions[0].operands_changed, Some(first != later));
        }
    }

    #[test]
    fn test_adjacent_transitions_keep_a_correction_followed_by_a_new_break() {
        let (_, _, report) = run(vec![
            source("early.xlsx", "2024Q3", "Q2 2024", 0, 119, false),
            source("middle.xlsx", "2025Q2", "Q2 2024", 1, 120, false),
            source("late.xlsx", "2026Q2", "Q2 2024", 2, 118, false),
        ]);
        let h = report
            .histories
            .iter()
            .find(|h| h.family == IdentityFamily::TccFooting)
            .unwrap();
        assert_eq!(
            h.sequence.iter().map(|a| a.state).collect::<Vec<_>>(),
            ["iii", "i", "iii"]
        );
        assert_eq!(
            h.transitions.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TransitionKind::Corrected, TransitionKind::Broken]
        );
        assert_eq!((h.transitions[0].from, h.transitions[0].to), (0, 1));
        assert_eq!((h.transitions[1].from, h.transitions[1].to), (1, 2));
    }

    #[test]
    fn prop_file_order_does_not_change_3a_or_3c() {
        let sources = vec![
            source("last.xlsx", "2026Q3", "YTD Q3 2024", 2, 120, false),
            source("first.xlsx", "2024Q3", "YTD Sept 30 2024", 0, 119, false),
            source("middle.xlsx", "2025Q3", "YTD Q3 2024", 1, 118, false),
        ];
        let (_, _, expected) = run(sources.clone());
        for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 1, 0], [2, 0, 1]] {
            let (mut report, index, actual) =
                run(order.into_iter().map(|i| sources[i].clone()).collect());
            assert_eq!(actual, expected);
            report.instances.reverse();
            assert_eq!(aisc_consistency(&index, &report.instances), expected);
        }
    }

    #[test]
    fn test_ambiguous_first_vintage_is_not_arbitrarily_selected() {
        let (_, _, report) = run(vec![
            source("one.xlsx", "2024Q3", "Q2 2024", 0, 119, false),
            source("two.xlsx", "2024Q3", "Q2 2024", 1, 120, false),
            source("later.xlsx", "2025Q2", "Q2 2024", 2, 120, false),
        ]);
        assert!(report.first_registered.is_empty());
        assert_eq!(report.all_files.values().sum::<usize>(), 15);
        for h in report.histories {
            assert_eq!(h.ambiguous_vintages, ["2024Q3"]);
            assert!(
                h.transitions
                    .iter()
                    .all(|t| t.kind == TransitionKind::Reclassified)
            );
        }
    }

    #[test]
    fn test_missing_keys_remain_named_and_cannot_prove_persistence() {
        let (mut report, mut index, _) = pair(120, 120);
        index.cells.remove("early.xlsx:YTD Sept 30 2024!B5");
        let result = aisc_consistency(&index, &report.instances);
        let h = result
            .histories
            .iter()
            .find(|h| h.family == IdentityFamily::TccFooting)
            .unwrap();
        assert_eq!(h.transitions[0].kind, TransitionKind::Reclassified);
        assert_eq!(h.transitions[0].operands_changed, None);
        index.cells.remove("early.xlsx:YTD Sept 30 2024!B7");
        let result = aisc_consistency(&index, &report.instances);
        assert_eq!(result.unkeyed.len(), 1);
        assert_eq!(result.unkeyed[0].at, "early.xlsx:YTD Sept 30 2024!B7");
        assert_eq!(result.all_files, report.state_counts());
        report.instances.reverse();
        assert_eq!(aisc_consistency(&index, &report.instances), result);
    }

    #[test]
    fn test_consistency_json_counts_citations_and_escaping() {
        use fmine_public::consistency::UnkeyedIdentity;
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let (_, _, mut report) = pair(119, 120);
        let odd = "\"quoted\"\\path\n\t\u{0001}";
        report.histories[0].series.subject = odd.to_string();
        report.histories[0].sequence[0].at = odd.to_string();
        report.unkeyed.push(UnkeyedIdentity {
            at: odd.to_string(),
            family: IdentityFamily::TccFooting,
            reason: odd.to_string(),
        });
        let (_, _, ambiguous) = run(vec![
            source("one.xlsx", "2024Q3", "Q2 2024", 0, 119, false),
            source("two.xlsx", "2024Q3", "Q2 2024", 1, 120, false),
        ]);
        let input = format!(
            "[{},{},{}]",
            report.to_json(),
            AiscConsistency::default().to_json(),
            ambiguous.to_json()
        );
        // The repository's stdlib-only Python oracle environment supplies an independent JSON parser.
        let script = r#"
import json, math, sys
data, empty, ambiguous = json.load(sys.stdin)
assert data['schema'] == 'fmine.issuer_consistency.aisc.v1'
assert 'first publication' in data['caveat'] and 'Hidden sheets are excluded' in data['caveat']
within = data['within_vintage']
assert 'including not-published (np)' in within['denominator']
first = within['first_registered']['tcc_footing']
all_files = within['all_files']['tcc_footing']
assert first['n'] == 1 and first['states']['iii']['n'] == 1 and first['states']['i']['n'] == 0
assert all_files['n'] == 2 and all_files['states']['i']['n'] == all_files['states']['iii']['n'] == 1
for family in [first, all_files]:
    assert sum(s['n'] for s in family['states'].values()) == family['n']
    for state in family['states'].values():
        assert math.isclose(state['rate'], state['n'] / family['n'])
        assert 0 <= state['cp95'][0] <= state['rate'] <= state['cp95'][1] <= 1
odd = '"quoted"' + chr(92) + 'path' + chr(10) + chr(9) + chr(1)
h = data['transitions'][0]
assert h['subject'] == h['total']['at'] == h['first_registered']['at'] == h['sequence'][0]['at'] == odd
assert h['total']['sha256'] == h['sequence'][0]['sha256'] and len(h['total']['sha256']) == 64
assert h['steps'] == [{'from': 0, 'to': 1, 'kind': 'corrected', 'operands_changed': True}]
assert data['unkeyed'][0]['at'] == data['unkeyed'][0]['reason'] == odd
for category in ['first_registered', 'all_files']:
    for family in empty['within_vintage'][category].values():
        assert family['n'] == 0
        assert all(s == {'n': 0, 'rate': None, 'cp95': None} for s in family['states'].values())
assert empty['transitions'] == empty['unkeyed'] == []
assert all(h['first_registered'] is None and h['ambiguous_vintages'] == ['2024Q3']
           for h in ambiguous['transitions'])
"#;
        let mut child = Command::new("python3")
            .args(["-I", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Python JSON oracle");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write JSON");
        let result = child.wait_with_output().expect("JSON parse completed");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn test_clopper_pearson_known_answers_and_edge_cases() {
        // Equal-tailed exact-binomial limits: for 0/5, (1-p)^5 = 0.025; the 5/5 case is its reflection.
        let (lo, hi) = clopper_pearson_95(0, 5).unwrap();
        assert_eq!(lo, 0.0);
        assert!((hi - 0.5218237501049815).abs() < 1e-12, "{hi}");
        let (lo, hi) = clopper_pearson_95(5, 5).unwrap();
        assert!((lo - 0.4781762498950185).abs() < 1e-12, "{lo}");
        assert_eq!(hi, 1.0);
        let (lo, hi) = clopper_pearson_95(5, 10).unwrap();
        assert!((lo - 0.1870860284473985).abs() < 1e-12, "{lo}");
        assert!((hi - 0.8129139715526015).abs() < 1e-12, "{hi}");
        assert_eq!(clopper_pearson_95(0, 0), None);
        assert_eq!(clopper_pearson_95(6, 5), None);
        assert_eq!(clopper_pearson_95(1, 100_001), None);
        // Independent scipy.stats.beta.ppf(0.025, k, n-k+1) and ppf(0.975, k+1, n-k) values.
        for (k, n, lo, hi) in [
            (306, 313, 0.9544660123984987, 0.9909622423710333),
            (250, 313, 0.7499628440626522, 0.8417256657295312),
            (1, 313, 0.00008088429413409716, 0.01767132207515211),
            (1297, 2595, 0.48038651286898876, 0.5192285659200009),
        ] {
            let actual = clopper_pearson_95(k, n).unwrap();
            assert!(
                (actual.0 - lo).abs() < 1e-11 && (actual.1 - hi).abs() < 1e-11,
                "{k}/{n}: {actual:?}"
            );
        }
        for n in [1, 5, 10, 313, 519, 2_595] {
            for k in [0, 1, n / 2, n - 1, n] {
                let (lo, hi) = clopper_pearson_95(k, n).unwrap();
                let (rlo, rhi) = clopper_pearson_95(n - k, n).unwrap();
                let observed = k as f64 / n as f64;
                assert!(lo <= observed && observed <= hi, "{k}/{n}: {lo}..{hi}");
                assert!((lo + rhi - 1.0).abs() < 1e-10);
                assert!((hi + rlo - 1.0).abs() < 1e-10);
            }
        }
    }

    #[test]
    #[ignore = "needs all registered Barrick workbooks under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_part3_real_aisc_dedup_and_persisted() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let profile = load_issuer_profile(&root, "barrick").unwrap();
        let report = assess(
            &profile,
            Lane::Public,
            load_registered_sources(&profile, &root).unwrap(),
        );
        let index = key_aisc_operands(&profile, &report.sources, &report.instances);
        let consistency = aisc_consistency(&index, &report.instances);
        assert!(consistency.unkeyed.is_empty(), "{:?}", consistency.unkeyed);
        assert_eq!(consistency.all_files, report.state_counts());
        // FACT: independent census2(displayed=True), share_census and region_census reads, grouped by canonical
        // period, subject and family and selecting the earliest registry quarter. Adding region-map scope did
        // not change these counts. The reader did not inspect consistency.rs before supplying these answers.
        assert_eq!(consistency.histories.len(), 1_565);
        assert_eq!(consistency.first_registered.values().sum::<usize>(), 1_565);
        assert_eq!(consistency.all_files.values().sum::<usize>(), 2_595);
        assert!(
            consistency
                .histories
                .iter()
                .all(|h| h.ambiguous_vintages.is_empty())
        );
        for (family, expected) in [
            (IdentityFamily::TccFooting, [306, 0, 1, 0, 0, 6]),
            (IdentityFamily::AiscFooting, [306, 0, 1, 0, 0, 6]),
            (IdentityFamily::CosPerOunce, [250, 0, 0, 57, 0, 6]),
            (IdentityFamily::TccPerOunce, [307, 0, 0, 0, 0, 6]),
            (IdentityFamily::AiscPerOunce, [305, 2, 0, 0, 0, 6]),
        ] {
            let actual = ["i", "ii", "iii", "iv", "v", "np"].map(|state| {
                consistency
                    .first_registered
                    .get(&(family, state))
                    .copied()
                    .unwrap_or(0)
            });
            assert_eq!(actual, expected, "{}", family.as_str());
        }
        for (subject, family, cell) in [
            ("Kibali", IdentityFamily::TccFooting, "O11"),
            ("Porgera", IdentityFamily::AiscFooting, "L18"),
        ] {
            let h = consistency
                .histories
                .iter()
                .find(|h| {
                    h.series.subject == subject
                        && h.family == family
                        && h.series.valid_period.code() == "2024Q2"
                })
                .unwrap();
            assert_eq!(
                h.sequence
                    .iter()
                    .map(|a| (a.state, a.release.as_str()))
                    .collect::<Vec<_>>(),
                [("iii", "2024Q3"), ("iii", "2025Q2")]
            );
            assert!(
                h.sequence
                    .iter()
                    .all(|a| a.at.ends_with(&format!("!{cell}")) && a.sha256.len() == 64)
            );
            assert_eq!(h.transitions[0].kind, TransitionKind::Persisted);
        }
        let mut reversed = report.instances.clone();
        reversed.reverse();
        assert_eq!(aisc_consistency(&index, &reversed), consistency);
        println!(
            "{{\"instances_all\":{},\"instances_first_registered\":{},\"oracle_equal\":true}}",
            consistency.all_files.values().sum::<usize>(),
            consistency.first_registered.values().sum::<usize>()
        );
    }
}
