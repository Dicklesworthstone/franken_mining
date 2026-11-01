//! Declared technical-report cut-off inputs (BD-274). Numerical values live only in agreed readings.
//! Determinism: Exact (explicit roles and canonical declaration/input order).

use std::collections::BTreeSet;

use crate::sources::{DocumentRow, Fields, SourcesError, fail};
use crate::toml_subset::TomlDoc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    GramsPerTonne,
    OuncesPerShortTon,
}

impl Unit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GramsPerTonne => "g_per_t",
            Self::OuncesPerShortTon => "oz_per_short_ton",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryModel {
    ConstantPercent,
    /// R_percent(g) = intercept_percent - slope_g_per_t * TCM_percent / g_g_per_t.
    TcmOverGold,
}

impl RecoveryModel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConstantPercent => "constant_percent",
            Self::TcmOverGold => "tcm_over_gold",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputRole {
    Published,
    PublishedMetric,
    GoldPrice,
    Cost,
    RevenueDeduction,
    OunceDeduction,
    Recovery,
    RecoveryIntercept,
    RecoverySlope,
    Tcm,
}

impl InputRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::PublishedMetric => "published_metric",
            Self::GoldPrice => "gold_price",
            Self::Cost => "cost",
            Self::RevenueDeduction => "revenue_deduction",
            Self::OunceDeduction => "ounce_deduction",
            Self::Recovery => "recovery",
            Self::RecoveryIntercept => "recovery_intercept",
            Self::RecoverySlope => "recovery_slope",
            Self::Tcm => "tcm",
        }
    }

    #[must_use]
    pub const fn unit(self, output: Unit) -> &'static str {
        match self {
            Self::Published => output.as_str(),
            Self::PublishedMetric | Self::RecoverySlope => "g_per_t",
            Self::GoldPrice | Self::OunceDeduction => "usd_per_oz",
            Self::Cost => "usd_per_t",
            Self::RevenueDeduction | Self::Recovery | Self::RecoveryIntercept | Self::Tcm => {
                "percent"
            }
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "published" => Self::Published,
            "published_metric" => Self::PublishedMetric,
            "gold_price" => Self::GoldPrice,
            "cost" => Self::Cost,
            "revenue_deduction" => Self::RevenueDeduction,
            "ounce_deduction" => Self::OunceDeduction,
            "recovery" => Self::Recovery,
            "recovery_intercept" => Self::RecoveryIntercept,
            "recovery_slope" => Self::RecoverySlope,
            "tcm" => Self::Tcm,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CutoffInput {
    pub role: InputRole,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoffDeclaration {
    pub id: String,
    pub doc: String,
    pub subject: String,
    /// Explicit source context, e.g. effective_2023_12_31; not inferred to be an annual flow.
    pub period: String,
    /// The source's physical/model basis. No ownership or dry/wet conversion is performed.
    pub basis: String,
    /// A commodity-price assumption has its own source basis, separate from physical quantities.
    pub price_basis: String,
    pub unit: Unit,
    pub recovery: RecoveryModel,
    pub inputs: Vec<CutoffInput>,
}

impl CutoffDeclaration {
    /// Rechecks caller-constructed declarations as well as parsed ones. Source eligibility is assessed per row.
    pub fn validate(&self, documents: &[DocumentRow]) -> Result<(), SourcesError> {
        let bad = |reason: &str| SourcesError {
            line: 0,
            reason: format!("cutoff {}: {reason}", self.id),
        };
        if !super::key(&self.id)
            || [&self.subject, &self.period, &self.basis, &self.price_basis]
                .iter()
                .any(|s| s.trim().is_empty() || s.trim() != s.as_str())
            || documents.iter().filter(|d| d.file == self.doc).count() != 1
        {
            return Err(bad(
                "id, subject, period, physical and price bases, and one registered document are required",
            ));
        }
        let mut quantities = BTreeSet::new();
        for input in &self.inputs {
            if !super::key(&input.quantity) || !quantities.insert(&input.quantity) {
                return Err(bad(
                    "input quantities must be distinct keys; the same printed amount cannot be counted twice",
                ));
            }
        }
        let count = |role| self.inputs.iter().filter(|i| i.role == role).count();
        if count(InputRole::Published) != 1
            || count(InputRole::GoldPrice) != 1
            || count(InputRole::Cost) == 0
            || count(InputRole::PublishedMetric) > 1
            || (count(InputRole::PublishedMetric) == 1 && self.unit != Unit::OuncesPerShortTon)
        {
            return Err(bad(
                "needs one published cutoff, one gold price, at least one cost, and at most one metric equivalent for an oz/short-ton cutoff",
            ));
        }
        let roles = [
            InputRole::RecoveryIntercept,
            InputRole::RecoverySlope,
            InputRole::Tcm,
        ];
        let recovery_ok = match self.recovery {
            RecoveryModel::ConstantPercent => {
                count(InputRole::Recovery) == 1 && roles.iter().all(|r| count(*r) == 0)
            }
            RecoveryModel::TcmOverGold => {
                count(InputRole::Recovery) == 0 && roles.iter().all(|r| count(*r) == 1)
            }
        };
        if !recovery_ok {
            return Err(bad(
                "recovery roles must exactly match constant_percent or tcm_over_gold; no unused or duplicate model parameters",
            ));
        }
        Ok(())
    }
}

/// Parses only the two cut-off table kinds; the parent owns the common schema and other declarations.
pub(super) fn parse(doc: &TomlDoc) -> Result<Vec<CutoffDeclaration>, SourcesError> {
    let mut cutoffs = Vec::new();
    for row in doc.rows_of("cutoff") {
        let f = Fields::of_row(row);
        f.only(&[
            "id",
            "doc",
            "subject",
            "period",
            "basis",
            "price_basis",
            "unit",
            "recovery",
        ])?;
        let unit = match f.string("unit")?.0.as_str() {
            "g_per_t" => Unit::GramsPerTonne,
            "oz_per_short_ton" => Unit::OuncesPerShortTon,
            _ => return fail(row.line, "cutoff unit must be g_per_t or oz_per_short_ton"),
        };
        let recovery = match f.string("recovery")?.0.as_str() {
            "constant_percent" => RecoveryModel::ConstantPercent,
            "tcm_over_gold" => RecoveryModel::TcmOverGold,
            _ => return fail(row.line, "unsupported cutoff recovery model"),
        };
        cutoffs.push(CutoffDeclaration {
            id: f.string("id")?.0,
            doc: f.string("doc")?.0,
            subject: f.string("subject")?.0,
            period: f.string("period")?.0,
            basis: f.string("basis")?.0,
            price_basis: f.string("price_basis")?.0,
            unit,
            recovery,
            inputs: Vec::new(),
        });
    }
    for row in doc.rows_of("cutoff_input") {
        let f = Fields::of_row(row);
        f.only(&["cutoff", "role", "quantity"])?;
        let id = f.string("cutoff")?.0;
        let Some(cutoff) = cutoffs.iter_mut().find(|c| c.id == id) else {
            return fail(
                row.line,
                format!("cutoff_input names unknown cutoff {id:?}"),
            );
        };
        let Some(role) = InputRole::parse(&f.string("role")?.0) else {
            return fail(row.line, "unknown cutoff input role");
        };
        cutoff.inputs.push(CutoffInput {
            role,
            quantity: f.string("quantity")?.0,
        });
    }
    cutoffs.sort_by(|a, b| a.id.cmp(&b.id));
    for cutoff in &mut cutoffs {
        cutoff.inputs.sort();
    }
    Ok(cutoffs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table_identities::parse_table_identities;

    fn fixture() -> (String, Vec<DocumentRow>) {
        let mut text = "schema = \"fmine.table-identities.v1\"\nissuer = \"synthetic\"\n[[cutoff]]\nid = \"mine_cutoff\"\ndoc = \"Mine.pdf\"\nsubject = \"Invented Mine\"\nperiod = \"effective_2024_12_31\"\nbasis = \"full;resource\"\nprice_basis = \"resource_price\"\nunit = \"g_per_t\"\nrecovery = \"constant_percent\"\n".to_string();
        for role in ["published", "gold_price", "cost", "recovery"] {
            text.push_str(&format!("[[cutoff_input]]\ncutoff = \"mine_cutoff\"\nrole = \"{role}\"\nquantity = \"cutoff_{role}\"\n"));
        }
        let docs = vec![DocumentRow {
            file: "Mine.pdf".into(),
            dir: "documents".into(),
            kind: "technical_report".into(),
            url: "https://example.invalid/Mine.pdf".into(),
            url_basis: "invented".into(),
            published_at: "2025-03-01".into(),
            published_at_basis: "FACT: invented date".into(),
            pages: 10,
            bytes: 1,
            sha256: "a".repeat(64),
        }];
        (text, docs)
    }

    #[test]
    fn declared_inputs_are_selected_without_accepting_literal_values() {
        let (text, docs) = fixture();
        let registry = parse_table_identities(&text, "synthetic", &docs).unwrap();
        assert_eq!(registry.cutoffs.len(), 1);
        assert_eq!(
            registry.quantities_by_document()["Mine.pdf"],
            [
                "cutoff_cost",
                "cutoff_gold_price",
                "cutoff_published",
                "cutoff_recovery"
            ]
        );
        let literal = text.replace(
            "unit = \"g_per_t\"",
            "unit = \"g_per_t\"\npublished = \"5.66\"",
        );
        assert!(parse_table_identities(&literal, "synthetic", &docs).is_err());
    }

    #[test]
    fn ambiguous_missing_wrong_model_and_wrong_unit_roles_refuse() {
        let (text, docs) = fixture();
        for changed in [
            text.replace(
                "quantity = \"cutoff_cost\"",
                "quantity = \"cutoff_gold_price\"",
            ),
            text.replace("role = \"recovery\"", "role = \"recovery_intercept\""),
            text.replace(
                "recovery = \"constant_percent\"",
                "recovery = \"tcm_over_gold\"",
            ),
            text.replace("unit = \"g_per_t\"", "unit = \"oz_per_tonne\""),
            text.replace("cutoff = \"mine_cutoff\"", "cutoff = \"absent\""),
            format!(
                "{text}[[cutoff_input]]\ncutoff = \"mine_cutoff\"\nrole = \"published\"\nquantity = \"second_cutoff\"\n"
            ),
        ] {
            assert!(parse_table_identities(&changed, "synthetic", &docs).is_err());
        }
    }
}
