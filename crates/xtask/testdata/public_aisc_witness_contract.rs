// Cross-crate assurance extraction and exact-certificate contracts. Included at the private posing seam;
// the test names, corpus gate, source expectations and arithmetic assertions are unchanged.

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
        let checked = xf::check_conflict_set(&cells, std::slice::from_ref(&identity), &rules, set);
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
    let expected =
        crate::sources::load_expected_roots(&crate::sources::expected_roots_path(&root, "barrick"))
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
