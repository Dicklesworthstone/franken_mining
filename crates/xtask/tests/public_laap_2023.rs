//! bd-4wvl.6 (BD-340.S6) known answer: the three Latin America & Asia Pacific 2023 cost-of-sales-per-ounce rates
//! that B7 (EVQ-P0-1) and E817 list as having no public closure, checked against the region's two members
//! (Pueblo Viejo, Veladero) by sold weight. Footnote h states the rate on attributable cost of sales, which no
//! row prints, so the linear form is unavailable: the check is the McCormick relaxation of
//! `rate·Σwᵢ = Σ rateᵢ·wᵢ`, plus the weights' sum.
//!
//! Cells: row 19 "Ounces sold - attributable basis (000s ounces)" and row 20 "Cost of sales per ounce" (footnotes
//! h, i); columns K (Pueblo Viejo), L (Veladero) and M (the region), headers in row 4. The values are as displayed:
//! integers stored under the zero-decimal accounting format. Read twice before commit (2026-10-30): once by the
//! oracle's `scripts/oracle/formats.py`, once by a second agent's own zipfile/XML reader given only the locators.
//! All 18 values, both sha256 and the labels agree.
//!
//! The default test runs the kernel on these values. The ignored one re-reads the registered bytes (BD-339 job).
//! Logs carry locators, verdicts and digests, never a cell value (RISK-31).

// In a module under `cfg(test)` so the helpers get clippy's test allowances (unwrap, expect, panic).
#[cfg(test)]
mod laap {
    use std::path::PathBuf;

    use fmine_io::xlsx::{CellValue, Workbook, parse_a1_ref};
    use fmine_numerics::exact_feas::{
        Cell, Certificate, Closure, Construction, Convention, Identity, IdentityKind, Outcome, Rat,
        RateCells, Rules, Scope, certificate_digest, check_certificate, region_against_members,
        solve,
    };

    const Q3_2024: &str = "Barrick_Q3_2024_AISC.xlsx";
    const Q3_2024_SHA256: &str = "d063ea7c1cd667a6db9c9de1b3aa4cb0ff2dba4a8cb24f6ce889e56af2fd0ef7";
    const Q4_2024: &str = "Barrick_Q4_2024_AISC.xlsx";
    const Q4_2024_SHA256: &str = "f842696915b7a44d00194e01a3bdf4a76d1e52d0e31c2610a91be70afb4b1b4d";

    /// The cells of each sheet, in this order.
    const REFS: [&str; 6] = ["K19", "L19", "M19", "K20", "L20", "M20"];

    /// (file, sha256, sheet, values at `REFS`).
    const LAAP_2023: [(&str, &str, &str, [i64; 6]); 3] = [
        (
            Q3_2024,
            Q3_2024_SHA256,
            "Q3 2023",
            [77, 47, 124, 1_501, 1_376, 1_468],
        ),
        (
            Q3_2024,
            Q3_2024_SHA256,
            "YTD Sept 30 2023",
            [246, 136, 382, 1_356, 1_461, 1_411],
        ),
        (
            Q4_2024,
            Q4_2024_SHA256,
            "YTD Dec 31 2023",
            [335, 182, 517, 1_418, 1_440, 1_441],
        ),
    ];

    const OUNCES: &str = "aisc_ounces_sold";

    fn locator(file: &str, sheet: &str, cell: &str) -> String {
        format!("{file}:{sheet}!{cell}")
    }

    /// Component-sum for the ounces (the AISC tables' declared construction, E815), every value closed.
    fn rules() -> Rules {
        Rules::new(Convention::Closed).with(OUNCES, Construction::ComponentSum)
    }

    /// One sheet's cells at step 1 and its region-against-members identities.
    fn problem(
        file: &str,
        sha256: &str,
        sheet: &str,
        values: [i64; 6],
    ) -> (Vec<Cell>, Vec<Identity>) {
        let cells = REFS
            .iter()
            .zip(values)
            .map(|(r, v)| Cell {
                locator: locator(file, sheet, r),
                sha256: sha256.to_string(),
                token: v.to_string(),
                printed: Rat::from_integer(i128::from(v)),
                step: Rat::from_integer(1),
                scope: Scope::Visible,
            })
            .collect();
        let at = |r: &str| locator(file, sheet, r);
        let ids = region_against_members(
            &format!("laap:{sheet}"),
            OUNCES,
            Closure::MayUseUnpublished,
            Rat::from_integer(1_000),
            &RateCells {
                rate: at("M20"),
                weight: at("M19"),
                numerator: None,
            },
            &[
                RateCells {
                    rate: at("K20"),
                    weight: at("K19"),
                    numerator: None,
                },
                RateCells {
                    rate: at("L20"),
                    weight: at("L19"),
                    numerator: None,
                },
            ],
        );
        (cells, ids)
    }

    /// Each rate is refuted through the envelope with a certificate the checker accepts. The conflict is the rate's:
    /// the weights alone foot, and every certificate carries McCormick rows.
    #[test]
    fn test_laap_2023_fixture() {
        let r = rules();
        let mut refuted = 0;
        for (file, sha256, sheet, values) in LAAP_2023 {
            let (cells, ids) = problem(file, sha256, sheet, values);
            assert!(
                ids.iter()
                    .any(|i| matches!(i.kind, IdentityKind::WeightedMean { .. })),
                "{sheet}: not the weighted mean"
            );
            let weights_only: Vec<Identity> = ids
                .iter()
                .filter(|i| matches!(i.kind, IdentityKind::Sum { .. }))
                .cloned()
                .collect();
            let weight_cells: Vec<Cell> = cells
                .iter()
                .filter(|c| c.locator.ends_with("19"))
                .cloned()
                .collect();
            // Failure messages name the sheet and the verdict, never a witness or a value (RISK-31).
            let components = solve(&weight_cells, &weights_only, &r).expect("solves");
            assert!(
                matches!(components[..], [ref c] if matches!(c.outcome, Outcome::Feasible { .. })),
                "{sheet}: the weights do not foot"
            );

            let components = solve(&cells, &ids, &r).expect("solves");
            let [component] = &components[..] else {
                panic!("{sheet}: {} components", components.len());
            };
            let Outcome::Infeasible {
                certificate: certificate @ Certificate::Farkas { multipliers },
            } = &component.outcome
            else {
                let verdict = match component.outcome {
                    Outcome::NotRefuted => "not_refuted",
                    Outcome::Feasible { .. } => "feasible",
                    Outcome::Indeterminate { .. } => "indeterminate",
                    Outcome::Infeasible { .. } => "infeasible without Farkas multipliers",
                };
                panic!("{sheet}: not refuted: {verdict}");
            };
            assert!(
                multipliers.iter().any(|(row, _)| row.contains("#z")),
                "{sheet}: no McCormick row in the certificate"
            );
            check_certificate(&cells, &ids, &r, certificate).expect("the checker accepts");
            let digest = certificate_digest(&cells, &ids, &r, certificate).expect("digest");
            println!(
                "{{\"fixture\":\"LAAP-2023\",\"rate_cell\":\"{}\",\"sha256\":\"{sha256}\",\"path\":\"mccormick\",\
                 \"verdict\":\"refuted\",\"certificate_digest\":\"{digest}\"}}",
                locator(file, sheet, "M20")
            );
            refuted += 1;
        }
        println!("{{\"rates\":3,\"refuted\":{refuted},\"not_refuted\":0}}");
        assert_eq!(refuted, 3);
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The fixture against the registered bytes: each file's sha256, the row labels and column headers, and every
    /// value as displayed.
    #[test]
    #[ignore = "needs Barrick_Q3_2024_AISC.xlsx and Barrick_Q4_2024_AISC.xlsx under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_laap_2023_fixture_cells_match_registered_bytes() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus_of_input_docs/downloaded/mine_stats");
        let text = |wb: &Workbook, sheet: &str, r: &str| -> String {
            let (row, col) = parse_a1_ref(r).expect("a1");
            match wb
                .sheet(sheet)
                .and_then(|s| s.cell(row, col))
                .map(|c| &c.value)
            {
                Some(CellValue::Text(t)) => t.trim().to_string(),
                _ => panic!("{sheet}!{r}: not a text cell"),
            }
        };
        let mut matched = 0;
        for (file, sha256, sheet, values) in LAAP_2023 {
            let wb = Workbook::from_file(&dir.join(file)).expect("registered workbook present");
            assert_eq!(hex(&wb.sha256), sha256, "{file}");
            assert!(text(&wb, sheet, "A19").starts_with("Ounces sold - attributable basis"));
            assert!(text(&wb, sheet, "A20").starts_with("Cost of sales per ounce"));
            assert_eq!(text(&wb, sheet, "K4"), "Pueblo Viejo");
            assert_eq!(text(&wb, sheet, "L4"), "Veladero");
            assert_eq!(text(&wb, sheet, "M4"), "Latin America & Asia Pacific");
            for (r, v) in REFS.iter().zip(values) {
                let (row, col) = parse_a1_ref(r).expect("a1");
                let cell = wb
                    .sheet(sheet)
                    .and_then(|s| s.cell(row, col))
                    .expect("cell present");
                let shown = cell
                    .displayed()
                    .expect("numeric")
                    .expect("displayed")
                    .text();
                // The message names the cell, never its value (RISK-31).
                assert!(
                    shown == v.to_string(),
                    "{}: the displayed value differs from the fixture",
                    locator(file, sheet, r)
                );
                println!(
                    "{{\"fixture\":\"LAAP-2023\",\"cell\":\"{}\",\"matches\":true}}",
                    locator(file, sheet, r)
                );
                matched += 1;
            }
        }
        println!("{{\"fixture\":\"LAAP-2023\",\"cells\":18,\"matched\":{matched}}}");
        assert_eq!(matched, 18);
    }
}
