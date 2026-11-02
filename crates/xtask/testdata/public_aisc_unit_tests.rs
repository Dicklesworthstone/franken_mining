#[cfg(test)]
mod tests {
    use super::*;

    // Public AISC integration checks live in xtask/tests/public_report_contract.rs.

    /// Column K of a one-sheet workbook: `(row, stored text, number format id)`, formats listed as `custom`.
    fn styled_column(cells: &[(u32, &str, u32)], custom: &[(u32, &str)]) -> Sheet {
        use fmine_io::xlsx::{
            MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
        };
        let mut xfs: Vec<u32> = Vec::new();
        let cells = cells
            .iter()
            .map(|&(row, raw, fmt)| {
                let xf = xfs.iter().position(|&f| f == fmt).unwrap_or_else(|| {
                    xfs.push(fmt);
                    xfs.len() - 1
                });
                let raw = raw.to_string();
                (
                    format!("K{row}"),
                    MinimalCell::Styled { raw, xf: xf as u32 },
                )
            })
            .collect();
        let styles = MinimalStyles {
            num_fmts: custom.iter().map(|&(id, c)| (id, c.to_string())).collect(),
            xfs,
        };
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "S".to_string(),
                hidden: false,
                cells,
            }],
            Some(&styles),
        );
        let wb = Workbook::from_bytes("t.xlsx", &bytes).expect("parses");
        wb.sheet("S").expect("sheet").clone()
    }

    /// TCC footing of column K (11) over lines 5–8 and the total at row 10, read through `val`.
    fn k_footing(sheet: &Sheet) -> (IdentityState, Calc) {
        let v = |r: u32| val(sheet, r, 11);
        let cell_ref = |r: u32| format!("K{r}");
        let format_at = |r: u32| {
            let code = sheet
                .cell(r, 11)
                .map(|c| c.format.code())
                .unwrap_or_default();
            format!("K{r} ({code})")
        };
        let lines = [5, 6, 7, 8];
        let family = IdentityFamily::TccFooting;
        footing_state(
            &v, &cell_ref, &format_at, 10, &lines, family, &lines, &lines,
        )
    }

    /// E823's quoted values under `#,##0` (id 3): 241 − 38 − 37.899643 − 33 = 132.100357 stored.
    fn e823_column() -> Sheet {
        styled_column(
            &[
                (5, "241", 3),
                (6, "-38", 3),
                (7, "-37.899643", 3),
                (8, "-33", 3),
                (10, "132.100357", 3),
            ],
            &[],
        )
    }

    /// bd-jg43.3: on displayed values E823's footing closes in integers, 241 − 38 − 38 − 33 = 132 against a
    /// printed 132, so it is (i) where the stored values were refused.
    #[test]
    fn test_e823_tcc_footing_fixture_scores_on_displayed() {
        let (state, calc) = k_footing(&e823_column());
        assert_eq!(
            state,
            IdentityState::Consistent {
                alternate_consistent: true,
                basis: None
            }
        );
        assert_eq!(
            calc,
            Calc::Footing {
                printed: 132,
                component_sum: 132,
                k_alt: 4
            }
        );
    }

    /// The E823 fixture tells the displayed reading from two wrong ones: the stored doubles (the ledger before
    /// bd-jg43.3) refuse it, and truncating instead of rounding (−37.899643 → −37) makes it a discrepancy.
    #[test]
    fn test_mutation_ledger_on_stored_value_fails_e823_fixture() {
        let sheet = e823_column();
        let stored = |r: u32| match sheet.cell(r, 11).map(|c| &c.value) {
            Some(CellValue::Number(x)) => Val::Num(*x),
            _ => Val::Blank,
        };
        let truncated = |r: u32| match sheet.cell(r, 11).map(|c| &c.value) {
            Some(CellValue::Number(x)) => Val::Num(x.trunc()),
            _ => Val::Blank,
        };
        let cell_ref = |r: u32| format!("K{r}");
        let lines = [5, 6, 7, 8];
        let family = IdentityFamily::TccFooting;
        let run = |v: &dyn Fn(u32) -> Val| {
            footing_state(v, &cell_ref, &cell_ref, 10, &lines, family, &lines, &lines).0
        };
        assert_eq!(run(&stored).code(), "v");
        assert_eq!(run(&truncated).code(), "iii");
        assert_eq!(k_footing(&sheet).0.code(), "i");
    }

    /// A zero line shown as the zero section's dash is the number 0 (BD-332 keeps the token itself): the footing
    /// sums it as 0 and counts it as a printed line.
    #[test]
    fn test_zero_token_counts_as_zero_in_footing() {
        let sheet = styled_column(
            &[
                (5, "300", 164),
                (6, "-50", 164),
                (7, "0", 164),
                (8, "-20", 164),
                (10, "230", 164),
            ],
            &[(164, "#,##0;(#,##0);\"-\"")],
        );
        assert_eq!(val(&sheet, 7, 11), Val::Num(0.0));
        let (state, calc) = k_footing(&sheet);
        assert_eq!(state.code(), "i");
        assert_eq!(
            calc,
            Calc::Footing {
                printed: 230,
                component_sum: 230,
                k_alt: 4
            }
        );
    }

    /// A line shown with a decimal (`#,##0.0`) is not a whole displayed number: the footing is refused, naming
    /// the cell and its format code.
    #[test]
    fn test_d_gt_0_operand_refused_with_reason() {
        let sheet = styled_column(
            &[
                (5, "300", 3),
                (6, "-50.4", 164),
                (7, "0", 3),
                (8, "-20", 3),
                (10, "230", 3),
            ],
            &[(164, "#,##0.0")],
        );
        let (state, calc) = k_footing(&sheet);
        assert_eq!(
            state,
            IdentityState::RefusedAt {
                reason: RefusedReason::DecimalsShown,
                at: "K6 (#,##0.0)".to_string(),
            }
        );
        assert_eq!(
            state.detail(),
            "decimals displayed; the declared construction sums whole displayed numbers: K6 (#,##0.0)"
        );
        assert_eq!(calc, Calc::None);
    }

    /// [`rate_holds`] at a share, decided.
    fn holds_at(n: i64, k: i64, q: i64, r: i64, share: (i64, i64)) -> bool {
        rate_holds(n, k, q, r, share).expect("the kernel decides")
    }

    /// [`rate_holds`] at 100%, decided.
    fn holds(n: i64, k: i64, q: i64, r: i64) -> bool {
        holds_at(n, k, q, r, (1, 1))
    }

    #[test]
    fn rate_interval_known_answers() {
        // 1000·79/68 = 1161.76…; with N ± 1/2 and Q ± 1/2 the range is [1145.985…, 1177.777…].
        assert!(holds(79, 1, 68, 1162));
        assert!(holds(79, 1, 68, 1146));
        assert!(!holds(79, 1, 68, 1141));
        assert_eq!(admitted_rates(79, 1, 68), Ok((1146, 1178)));
        // Outward rounding: 1177.777… prints as 1177.778 (it printed 1177.777 before 2026-10-29, below the bound).
        assert_eq!(implied_rate_text(79, 1, 68), "[1145.985, 1177.778]");
        // bd-sjrh.1, NEW-cell-number-formats' worked example: 129500/69.5 = 1863.3093… and 134500/68.5 =
        // 1963.5036…; 127500/69.5 = 1834.5323… and 138500/68.5 = 2021.8978….
        assert_eq!(implied_rate_text(132, 5, 69), "[1863.309, 1963.504]");
        assert_eq!(implied_rate_text(133, 11, 69), "[1834.532, 2021.898]");
        // Negative range: floor of −578.94… is −578.948; ceiling of −428.57… is −428.571.
        assert_eq!(implied_rate_text(-5, 1, 10), "[-578.948, -428.571]");
        // Negative numerators: 1000·(−5 ± ½)/(10 ± ½) spans [−578.9…, −428.5…].
        assert!(holds(-5, 1, 10, -500));
        assert!(!holds(-5, 1, 10, 500));
        // k widens the numerator: 1000·(100 ± 2)/(100 ± ½) reaches 1025.125…, which meets [1024.5, 1025.5]
        // but not [1026.5, 1027.5].
        assert!(holds(100, 4, 100, 1025));
        assert!(!holds(100, 4, 100, 1027));
        // k = 0 (every line a placeholder): N is exact. 1000·100/(100 ± ½) = [995.02…, 1005.02…] holds 1005
        // ([1004.5, 1005.5] meets it) and not 1006; with k = 1 the range reaches 1010.05… and holds 1006.
        assert!(holds(100, 0, 100, 1005));
        assert!(!holds(100, 0, 100, 1006));
        assert!(holds(100, 1, 100, 1006));
    }

    /// For a (iii) footing: BD-340.S3's conflict sets for the footing as the ledger poses it (from its cited
    /// operands; a placeholder is an exact 0), and the certificate's own cells, total first. Under the component
    /// sum, disregarding any one printed value drops the footing, so the one minimal set is every cell.
    fn footing_conflicts(inst: &IdentityInstance) -> (Vec<Vec<String>>, Vec<String>) {
        let IdentityState::IssuerDiscrepancy(c) = &inst.state else {
            panic!("{}: not (iii)", inst.cell)
        };
        let (mut cells, mut lines) = (Vec::new(), Vec::new());
        for o in &inst.operands {
            let (value, step) = match &o.value {
                OperandValue::Int(x) => (*x, 1),
                OperandValue::Token(_) => (0, 0),
                other => panic!("{}: {other:?}", o.cell),
            };
            match o.role {
                OperandRole::Total => cells.insert(0, xf_cell(&o.cell, value, step)),
                OperandRole::Line => {
                    lines.push(o.cell.clone());
                    cells.push(xf_cell(&o.cell, value, step));
                }
                _ => {}
            }
        }
        let identity = xf::Identity {
            id: "footing".to_string(),
            family: "footing".to_string(),
            closure: xf::Closure::Definitional,
            kind: xf::IdentityKind::Sum {
                total: c.total_cell.clone(),
                signs: vec![1; lines.len()],
                lines,
            },
        };
        let rules =
            xf::Rules::new(xf::Convention::Closed).with("footing", xf::Construction::ComponentSum);
        let found = xf::blame(&cells, std::slice::from_ref(&identity), &rules).expect("blames");
        let [component] = found.as_slice() else {
            panic!("{}: {found:?}", inst.cell)
        };
        let xf::BlameOutcome::Blamed(b) = &component.outcome else {
            panic!("{}: {:?}", inst.cell, component.outcome)
        };
        for set in &b.conflicts {
            let checked =
                xf::check_conflict_set(&cells, std::slice::from_ref(&identity), &rules, set);
            assert!(checked.is_ok(), "{}: {checked:?}", inst.cell);
        }
        let mut own: Vec<String> = std::iter::once(c.total_cell.clone())
            .chain(c.line_cells.iter().cloned())
            .collect();
        own.sort();
        (b.conflicts.iter().map(|s| s.cells.clone()).collect(), own)
    }

    /// (iii) carries S3's minimal set and a witness derived from the residual certificate: synthetic C8 prints 350
    /// against 250, and flipping C6 (−50) closes it.
    #[test]
    fn test_sign_flip_witness_from_certificate_c8_c6() {
        let run = crate::run_synthetic_assurance(&[]).expect("synthetic run");
        let discrepancies: Vec<&IdentityInstance> = run
            .instances
            .iter()
            .chain(&run.hidden_scope.instances)
            .filter(|i| matches!(i.state, IdentityState::IssuerDiscrepancy(_)))
            .collect();
        assert!(!discrepancies.is_empty());
        for inst in &discrepancies {
            let (conflicts, own) = footing_conflicts(inst);
            assert_eq!(conflicts, vec![own], "{}", inst.cell);
        }
        let c8 = discrepancies
            .iter()
            .find(|i| i.cell == "C8" && i.family == IdentityFamily::TccFooting)
            .expect("C8");
        let IdentityState::IssuerDiscrepancy(c) = &c8.state else {
            panic!("C8")
        };
        assert_eq!((c.printed, c.residual), (350, 100));
        assert_eq!(c.sign_flip_witness.as_deref(), Some("C6"));
        assert_eq!(footing_conflicts(c8).0, vec![vec!["C5", "C6", "C7", "C8"]]);
    }

    /// 10⁴ seeded footings with placeholder lines against the closed-form rules the corner kernel used: the
    /// primary holds iff the printed lines sum to the printed total, the alternate iff |printed − sum|·2 ≤ 1 + k.
    /// The rates' closed-form oracle is `xtask/tests/public_exact_feas_oracle.rs`. The BD-340 S7 table generator does not
    /// exist yet, so these stand in for its tables.
    #[test]
    fn test_property_footings_match_closed_form() {
        use fmine_core::{PhiloxStream, RngKey};
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 331,
            partition: 6,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |lo: i64, hi: i64| lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64;
        let mut footing_alt_only = 0;
        for case in 0..10_000 {
            // A footing of 1–6 lines, some placeholders (exact 0), printed near its sum.
            let lines: Vec<Option<i64>> = (0..next(1, 6))
                .map(|_| (next(0, 3) > 0).then(|| next(-500, 500)))
                .collect();
            let sum: i64 = lines.iter().flatten().sum();
            let printed = sum + next(-3, 3);
            let k_alt = lines.iter().flatten().count() as i64;
            let terms: Vec<(String, i64, i64)> = lines
                .iter()
                .enumerate()
                .map(|(i, l)| (format!("L{i}"), l.unwrap_or(0), i64::from(l.is_some())))
                .collect();
            let primary = footing_decision(("T", printed), &terms, xf::Construction::ComponentSum)
                .expect("decided");
            let alternate = footing_decision(("T", printed), &terms, xf::Construction::RoundedOnce)
                .expect("decided");
            assert_eq!(primary.is_none(), printed == sum, "case {case}");
            let old_alt = (printed - sum).abs() * 2 <= 1 + k_alt;
            assert_eq!(
                alternate.is_none(),
                old_alt,
                "case {case}: {lines:?} {printed}"
            );
            footing_alt_only += usize::from(old_alt && printed != sum);
        }
        println!(
            "{{\"test\":\"footing_property\",\"cases\":10000,\"seed\":331,\
             \"footing_alternate_only\":{footing_alt_only},\"disagree\":0}}"
        );
        assert!(footing_alt_only > 500, "{footing_alt_only}");
    }

    /// The alternative construction survives as a `RoundedOnce` run: TCC, as a line of the AISC footing, stands
    /// for its k₁ numeric block-1 lines (TCC ± k₁/2), as `aisc.rs` always counted it.
    #[test]
    fn test_alternate_consistent_preserved() {
        let vals: BTreeMap<u32, Val> = [
            (1, Val::Num(50.0)),
            (2, Val::Num(50.0)),
            (3, Val::Num(101.0)), // TCC, against 100
            (4, Val::Num(10.0)),
            (5, Val::Place),
            (6, Val::Num(113.0)), // AISC, against 101 + 10 = 111
        ]
        .into_iter()
        .collect();
        let v = |r: u32| vals.get(&r).copied().unwrap_or(Val::Blank);
        let cell_ref = |r: u32| format!("C{r}");
        let (block1, block2) = ([1, 2], [4, 5]);
        let all: Vec<u32> = block1.iter().chain(&block2).copied().collect();
        let footing = |total, lines: &[u32], family| {
            footing_state(
                &v, &cell_ref, &cell_ref, total, lines, family, &block1, &all,
            )
            .0
        };
        // TCC: residual 1 within (1 + 2)/2.
        let IdentityState::IssuerDiscrepancy(tcc) = footing(3, &block1, IdentityFamily::TccFooting)
        else {
            panic!("TCC footing")
        };
        assert_eq!((tcc.residual, tcc.alternate_consistent), (1, true));
        // AISC: residual 2 within (1 + 2 + 1)/2, which needs TCC ± 2/2; a TCC rounded once (± ½) would miss it.
        let IdentityState::IssuerDiscrepancy(aisc) =
            footing(6, &[3, 4, 5], IdentityFamily::AiscFooting)
        else {
            panic!("AISC footing")
        };
        assert_eq!((aisc.residual, aisc.alternate_consistent), (2, true));
        let rounded_once_tcc = [
            ("C3".to_string(), 101, 1),
            ("C4".to_string(), 10, 1),
            ("C5".to_string(), 0, 0),
        ];
        assert!(
            footing_decision(
                ("C6", 113),
                &rounded_once_tcc,
                xf::Construction::RoundedOnce
            )
            .expect("decided")
            .is_some()
        );
        // Rates: the alternate is the same rate with one half-width (N rounded once). 1000·(100 ± 2)/(100 ± ½)
        // reaches 1025.125…; 1000·(100 ± ½)/(100 ± ½) only 1010.05….
        assert!(holds(100, 4, 100, 1025) && !holds(100, 1, 100, 1025));
    }

    /// BD-331.S6 on the registered Barrick workbooks through 2026Q2. At cd423b9 the two kernels ran side by side
    /// on all 3,030 instances with no difference; the corner kernel is gone, and the root pins the result:
    /// `root_old` is the last row of `repro/barrick/expected.toml` (the corner kernel's until bd-jg43.3 moved
    /// the root on purpose, at 2386e66). Every (iii) carries
    /// S3's one minimal set. Prints locators and counts only (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_port_keeps_root_and_minimal_sets() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let profile = crate::sources::load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let run = crate::run_public_assurance(&profile, &root)
            .expect("registered files present and unaltered");
        let expected = crate::sources::load_expected_roots(&crate::sources::expected_roots_path(
            &root, "barrick",
        ))
        .expect("expected roots");
        let root_old = &expected.last().expect("a row").state_root;
        let all: Vec<&IdentityInstance> = run
            .instances
            .iter()
            .chain(&run.hidden_scope.instances)
            .collect();
        let root_new = run.ledger_anchor.state_root.to_hex();
        println!(
            "{{\"instances\":{},\"visible\":{},\"root_old\":\"{root_old}\",\"root_new\":\"{root_new}\"}}",
            all.len(),
            run.instances.len()
        );
        assert_eq!((all.len(), run.instances.len()), (3030, 2595));
        assert_eq!(&root_new, root_old);
        // Every (iii) carries S3's minimal set; Kibali 'Q2 2024'!O11 (E815) has the witness O7.
        let mut discrepancies = 0;
        for inst in all
            .iter()
            .filter(|i| matches!(i.state, IdentityState::IssuerDiscrepancy(_)))
        {
            let (conflicts, own) = footing_conflicts(inst);
            assert_eq!(
                conflicts,
                vec![own],
                "{}:{}!{}",
                inst.file,
                inst.sheet,
                inst.cell
            );
            discrepancies += 1;
        }
        let kibali = all
            .iter()
            .find(|i| {
                i.file == "Barrick_Q3_2024_AISC.xlsx"
                    && i.sheet == "Q2 2024"
                    && i.cell == "O11"
                    && i.family == IdentityFamily::TccFooting
            })
            .expect("Kibali O11");
        let IdentityState::IssuerDiscrepancy(c) = &kibali.state else {
            panic!("Kibali O11 is not (iii)")
        };
        assert_eq!(c.sign_flip_witness.as_deref(), Some("O7"));
        println!("{{\"discrepancies_with_minimal_set\":{discrepancies}}}");
    }

    /// A placeholder line is an exact 0: it never widens a footing's alternate, while a printed 0 does.
    #[test]
    fn test_placeholder_line_not_in_k() {
        let terms = |placeholder: bool| -> Vec<(String, i64, i64)> {
            (0..5)
                .map(|i| {
                    let value = if i < 2 { 50 } else { 0 };
                    let step = i64::from(i < 2 || !placeholder);
                    (format!("L{i}"), value, step)
                })
                .collect()
        };
        // Printed 102 against 100: the alternate needs |2| ≤ (1 + k)/2.
        let alternate = |placeholder| {
            footing_decision(
                ("T", 102),
                &terms(placeholder),
                xf::Construction::RoundedOnce,
            )
            .expect("decided")
            .is_none()
        };
        assert!(!alternate(true), "three placeholders: k = 2, (1 + 2)/2 < 2");
        assert!(
            alternate(false),
            "three printed zeros: k = 5, (1 + 5)/2 ≥ 2"
        );
        // The same through the ledger: lines 50, 50, —, —, — and TCC 102.
        let vals: BTreeMap<u32, Val> = [
            (1, Val::Num(50.0)),
            (2, Val::Num(50.0)),
            (3, Val::Place),
            (4, Val::Place),
            (5, Val::Place),
            (6, Val::Num(102.0)),
        ]
        .into_iter()
        .collect();
        let v = |r: u32| vals.get(&r).copied().unwrap_or(Val::Blank);
        let cell_ref = |r: u32| format!("C{r}");
        let block1 = [1, 2, 3, 4, 5];
        let (state, calc) = footing_state(
            &v,
            &cell_ref,
            &cell_ref,
            6,
            &block1,
            IdentityFamily::TccFooting,
            &block1,
            &block1,
        );
        assert!(matches!(calc, Calc::Footing { k_alt: 2, .. }), "{calc:?}");
        let IdentityState::IssuerDiscrepancy(c) = state else {
            panic!("{state:?}")
        };
        assert_eq!((c.residual, c.alternate_consistent), (2, false));
        assert_eq!(c.line_cells, vec!["C1", "C2", "C3", "C4", "C5"]);
    }

    #[test]
    fn test_numeric_zero_line_counts_in_k() {
        // TCC footing with lines 50, 50, 0, 0, 0 printed as numbers, total 102: k_alt = 5 and the alternate holds.
        let vals: BTreeMap<u32, Val> = [
            (1, Val::Num(50.0)),
            (2, Val::Num(50.0)),
            (3, Val::Num(0.0)),
            (4, Val::Num(0.0)),
            (5, Val::Num(0.0)),
            (6, Val::Num(102.0)),
        ]
        .into_iter()
        .collect();
        let v = |r: u32| vals.get(&r).copied().unwrap_or(Val::Blank);
        let cell_ref = |r: u32| format!("C{r}");
        let block1 = [1, 2, 3, 4, 5];
        let (state, calc) = footing_state(
            &v,
            &cell_ref,
            &cell_ref,
            6,
            &block1,
            IdentityFamily::TccFooting,
            &block1,
            &block1,
        );
        assert!(matches!(calc, Calc::Footing { k_alt: 5, .. }), "{calc:?}");
        let IdentityState::IssuerDiscrepancy(c) = state else {
            panic!("{state:?}")
        };
        assert!(c.alternate_consistent);
    }

    /// The bead's mutation: n = 200 from two numeric lines and one placeholder, q = 100, r = 2021. At k = 2 the
    /// range is [1980.10, 2020.10] and the rate misses; counting the placeholder (k = 3) reaches 2025.13 and would
    /// wrongly pass.
    #[test]
    fn test_mutation_placeholder_counted_in_k_flips_verdict() {
        assert_eq!(implied_rate_text(200, 2, 100), "[1980.099, 2020.101]");
        assert!(!holds(200, 2, 100, 2021));
        assert!(holds(200, 3, 100, 2021));
    }

    /// D16 (synthetic): N = 200 ± 5/2, Q = 100 ± ½, so 1000·N/Q spans [395000/201, 405000/199], shown outward.
    #[test]
    fn test_d16_implied_interval_outward() {
        let iv = rate_range(200, 5, 100).expect("a range");
        assert_eq!(
            (iv.lo, iv.hi),
            (
                Rat::new(395_000, 201).expect("lo"),
                Rat::new(405_000, 199).expect("hi")
            )
        );
        assert_eq!(implied_rate_text(200, 5, 100), "[1965.174, 2035.176]");
        // E691, Veladero cost of sales per ounce ('Q4 2025'!J19): N = 67 ± ½, Q = 47 ± ½ spans [1400, 45000/31].
        let j19 = rate_range(67, 1, 47).expect("a range");
        assert_eq!(
            (j19.lo, j19.hi),
            (Rat::from_integer(1400), Rat::new(45_000, 31).expect("hi"))
        );
        assert_eq!(implied_rate_text(67, 1, 47), "[1400.000, 1451.613]");
    }

    // Workbook and ownership-label checks live in xtask/tests/public_keys_aisc.rs; keep the private
    // decimal parser cases here.
    #[test]
    fn test_share_label_confirmation() {
        assert_eq!(decimal_fraction("89.7"), Some((897, 10)));
        assert_eq!(decimal_fraction("84"), Some((84, 1)));
        for bad in ["", ".5", "8.4.1", "61.55555", "-5", "x"] {
            assert_eq!(decimal_fraction(bad), None, "{bad}");
        }
    }
}
