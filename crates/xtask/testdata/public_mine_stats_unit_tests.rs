#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::load_issuer_profile;

    // Cross-crate XLSX fixtures and reader checks live in xtask/tests/public_workbook_reading.rs.

    #[test]
    fn test_label_rules() {
        assert_eq!(
            strip_footnote("Total cash costs3"),
            ("Total cash costs".to_string(), Some(3))
        );
        assert_eq!(
            strip_footnote("  Gold  produced (000s oz)12 "),
            ("Gold produced (000s oz)".to_string(), Some(12))
        );
        assert_eq!(
            strip_footnote("Q2 2026"),
            ("Q2 2026".to_string(), None),
            "digits after a space stay"
        );
        assert_eq!(
            strip_footnote("Tonnes (000s)"),
            ("Tonnes (000s)".to_string(), None)
        );
        assert_eq!(
            strip_footnote("Grade123"),
            ("Grade123".to_string(), None),
            "three digits are not a footnote"
        );
        assert_eq!(
            strip_footnote("Total\u{a0}cash costs 4"),
            ("Total cash costs 4".to_string(), None)
        );
        assert_eq!(site_name("Long Canyon (61.5%)1"), "Long Canyon");
        assert_eq!(site_name("Hemlo3"), "Hemlo");
        assert_eq!(site_name("Copper - Total"), "Total Copper");
        assert_eq!(site_name("Zaldívar (50%)"), "Zaldívar");
        assert_eq!(
            printed_share("Nevada Gold Mines LLC (61.5%)2"),
            Some("61.5")
        );
        assert_eq!(
            printed_share("Lumwana"),
            None,
            "no share printed means 100%, recorded as unprinted"
        );
        assert_eq!(printed_share("Site (n/a%)"), None);
        assert!(footnote_line("1 Barrick's share") && footnote_line("  12\tNote"));
        assert!(!footnote_line("2026") && !footnote_line("Tonnes"));
        let field = |h: &str| summary_field(h).expect("maps");
        assert_eq!(
            field("Gold produced (000s oz 100%)").as_deref(),
            Some("au_prod_100_koz")
        );
        assert_eq!(
            field("Gold produced (000s oz attributable)3").as_deref(),
            Some("au_prod_attr_koz")
        );
        assert_eq!(
            field("Copper sold (thousands of tonnes attributable)").as_deref(),
            Some("cu_sold_attr_kt")
        );
        assert_eq!(
            field("Total Cost of sales ($/oz)").as_deref(),
            Some("cos_usd_per_oz")
        );
        assert_eq!(
            field("C1 Cash Costs ($/lb)4").as_deref(),
            Some("c1_usd_per_lb")
        );
        assert_eq!(
            field("All-in sustaining costs ($/lb)").as_deref(),
            Some("aisc_usd_per_lb")
        );
        assert_eq!(field("Site"), None);
        assert!(
            summary_field("Total cash costs").is_err(),
            "a field with no metal is refused"
        );
    }

    /// The registry decides which rows are sub-rows; W1's list, used without a vocabulary, must say the same.
    #[test]
    fn test_child_rows_equal_the_registered_sections() {
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let registered: BTreeSet<String> = profile
            .mine_stats_labels
            .iter()
            .filter(|(section, _)| section.is_some())
            .map(|(_, label)| label.to_lowercase())
            .collect();
        let code: BTreeSet<String> = CHILD_ROWS.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(registered, code);
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }
}
