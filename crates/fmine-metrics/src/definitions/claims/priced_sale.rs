//! Exact gross-sales price linkage for declared royalty and by-product cost lines (plan §21.6).
//! Determinism: Exact

use fmine_core::ObjectId;
use fmine_evidence::RefusalCode;
use fmine_units::{Currency, ExactQty, Money};

use super::super::declared::{js, qty};
use super::super::geo::Metal;
use super::{CostUnit, ONE, Result, bad, exact};

pub(super) const ROYALTY_ITEM: &str = "royalties_and_production_taxes";
pub(super) const CREDIT_ITEM: &str = "by_product_credits";

/// One period's sale at a declared price. Both fractions apply to the same gross sales.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricedSale {
    pub metal: Metal,
    pub quantity_sold: ExactQty,
    pub usd_per_unit: ExactQty,
    pub royalty_fraction: Option<ExactQty>,
    pub by_product_credit_fraction: Option<ExactQty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaleAssessment {
    pub sale: PricedSale,
    pub input_claim_id: ObjectId,
    pub gross_sales_usd: ExactQty,
    pub royalty: Option<Money>,
    pub by_product_credit: Option<Money>,
}

fn decimal(value: Option<ExactQty>) -> String {
    value.map_or_else(|| "null".into(), |q| js(&qty(q)))
}

impl PricedSale {
    pub(crate) fn json_fields(&self) -> String {
        format!(
            "\"metal\":{},\"unit\":{},\"quantity_sold\":{},\"usd_per_unit\":{},\"royalty_fraction\":{},\"by_product_credit_fraction\":{}",
            js(self.metal.name()),
            js(self.metal.unit()),
            js(&qty(self.quantity_sold)),
            js(&qty(self.usd_per_unit)),
            decimal(self.royalty_fraction),
            decimal(self.by_product_credit_fraction)
        )
    }

    pub(super) fn assess(
        &self,
        unit: CostUnit,
        sold: ExactQty,
        input_claim_id: ObjectId,
    ) -> Result<SaleAssessment> {
        if unit != CostUnit::GoldOunceSold {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "gross-sale royalty and by-product credit mappings require a gold cost worksheet",
            ));
        }
        if self.metal == Metal::Gold
            && (self.quantity_sold != sold || self.by_product_credit_fraction.is_some())
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "gold sales must equal denominator ounces and cannot supply a by-product credit",
            ));
        }
        if self.quantity_sold.signum() < 0 || self.usd_per_unit.signum() <= 0 {
            return Err(bad(
                RefusalCode::OutsidePhysicalDomain109,
                "priced sales require nonnegative sold units and a positive price",
            ));
        }
        if self.royalty_fraction.is_none() && self.by_product_credit_fraction.is_none() {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "priced sale requires at least one declared royalty or by-product credit fraction",
            ));
        }
        let gross_sales_usd = exact(self.quantity_sold.checked_mul(self.usd_per_unit))?;
        let amount = |fraction: ExactQty| -> Result<Money> {
            if fraction.signum() < 0 || exact(fraction.checked_cmp(ONE))?.is_gt() {
                return Err(bad(
                    RefusalCode::OutsidePhysicalDomain109,
                    "gross-sales fractions must lie in [0, 1]",
                ));
            }
            let charge = exact(gross_sales_usd.checked_mul(fraction))?;
            let minor = exact(charge.checked_mul(ExactQty::from_integer(Money::SCALE)))?;
            if minor.den() != 1 {
                return Err(bad(
                    RefusalCode::NumericalCertificateFailed207,
                    "price-linked amount is not exactly representable at USD 0.0001",
                ));
            }
            Ok(Money::from_minor(Currency::USD, minor.num()))
        };
        Ok(SaleAssessment {
            sale: self.clone(),
            input_claim_id,
            gross_sales_usd,
            royalty: self.royalty_fraction.map(amount).transpose()?,
            by_product_credit: self.by_product_credit_fraction.map(amount).transpose()?,
        })
    }
}

impl SaleAssessment {
    pub(crate) fn to_json(&self) -> String {
        let minor = |amount: Option<Money>| {
            amount.map_or_else(|| "null".into(), |m| js(&m.minor.to_string()))
        };
        format!(
            "{{{},\"input_claim_id\":{},\"gross_sales_usd\":{},\"royalty_usd_minor\":{},\"by_product_credit_usd_minor\":{},\"scale\":10000}}",
            self.sale.json_fields(),
            js(&self.input_claim_id.to_string()),
            js(&qty(self.gross_sales_usd)),
            minor(self.royalty),
            minor(self.by_product_credit)
        )
    }
}
