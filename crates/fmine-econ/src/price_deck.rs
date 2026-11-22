//! Determinism: BitwisePortable
//! Versioned commodity price decks with `INV-013` staleness guards and royalty deck-mix detection
//! (`BD-228`, `BD-296`, plan §23.4, `FACT E244`).

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_units::{Currency, ExactQty, Money};

use crate::{EconError, money_mul_exact};

/// Covered base and precious commodities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Commodity {
    Au,
    Cu,
    Ag,
    Sn,
}

impl Commodity {
    /// Canonical symbol string.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Au => "Au",
            Self::Cu => "Cu",
            Self::Ag => "Ag",
            Self::Sn => "Sn",
        }
    }
}

/// Pricing denominator unit for a commodity price point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PriceUnit {
    PerTroyOz,
    PerAvdpLb,
    PerMetricTonne,
}

impl PriceUnit {
    const fn tag(self) -> u8 {
        match self {
            Self::PerTroyOz => 1,
            Self::PerAvdpLb => 2,
            Self::PerMetricTonne => 3,
        }
    }
}

/// One typed price entry inside a versioned [`PriceDeck`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricePoint {
    pub commodity: Commodity,
    pub unit: PriceUnit,
    pub price: Money,
}

impl PricePoint {
    /// Construct a validated non-negative commodity price point.
    pub fn new(commodity: Commodity, unit: PriceUnit, price: Money) -> Result<Self, EconError> {
        if price.minor < 0 {
            return Err(EconError::InvalidDomain {
                field: "price",
                detail: format!("negative price {} for {}", price.minor, commodity.symbol()),
            });
        }
        Ok(Self {
            commodity,
            unit,
            price,
        })
    }
}

/// Immutable versioned commodity price deck carrying canonical digest and `INV-013` supersession state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceDeck {
    pub deck_id: String,
    pub version: u32,
    pub as_of_date: String,
    pub currency: Currency,
    pub superseded_by: Option<String>,
    prices: BTreeMap<Commodity, PricePoint>,
    pub deck_digest: Digest32,
}

impl PriceDeck {
    /// Build a validated, digest-bound versioned [`PriceDeck`].
    pub fn build(
        deck_id: impl Into<String>,
        version: u32,
        as_of_date: impl Into<String>,
        currency: Currency,
        points: Vec<PricePoint>,
        superseded_by: Option<String>,
    ) -> Result<Self, EconError> {
        let deck_id = deck_id.into();
        let as_of_date = as_of_date.into();
        if deck_id.is_empty() || version == 0 || as_of_date.is_empty() || points.is_empty() {
            return Err(EconError::InvalidDomain {
                field: "price_deck",
                detail: "deck_id, version >= 1, as_of_date, and >= 1 price point are required"
                    .to_string(),
            });
        }
        let mut prices = BTreeMap::new();
        for pt in points {
            if pt.price.minor < 0 {
                return Err(EconError::InvalidDomain {
                    field: "price",
                    detail: format!(
                        "negative price {} for {} in price deck '{deck_id}'",
                        pt.price.minor,
                        pt.commodity.symbol()
                    ),
                });
            }
            if pt.price.ccy != currency {
                return Err(EconError::CurrencyMismatch {
                    left: currency,
                    right: pt.price.ccy,
                });
            }
            if prices.insert(pt.commodity, pt).is_some() {
                return Err(EconError::InvalidDomain {
                    field: "commodity",
                    detail: format!(
                        "duplicate commodity {} in price deck '{deck_id}'",
                        pt.commodity.symbol()
                    ),
                });
            }
        }

        let mut enc = Encoder::new_frame(*b"PDEC", 1);
        enc.put_str(&deck_id);
        enc.put_u32(version);
        enc.put_str(&as_of_date);
        enc.put_u8(currency as u8);
        match &superseded_by {
            Some(s) => {
                enc.put_u8(1);
                enc.put_str(s);
            }
            None => enc.put_u8(0),
        }
        enc.put_u32(prices.len() as u32);
        for pt in prices.values() {
            enc.put_str(pt.commodity.symbol());
            enc.put_u8(pt.unit.tag());
            enc.put_i128(pt.price.minor);
        }

        Ok(Self {
            deck_id,
            version,
            as_of_date,
            currency,
            superseded_by,
            prices,
            deck_digest: sha256(&enc.finish()),
        })
    }

    /// Enforce `INV-013`: refuse if this deck has been superseded by a newer version.
    pub fn check_fresh(&self) -> Result<(), EconError> {
        if let Some(ref next) = self.superseded_by {
            return Err(EconError::StaleVersion {
                kind: "price_deck",
                id: format!("{}@v{}", self.deck_id, self.version),
                superseded_by: next.clone(),
            });
        }
        Ok(())
    }

    /// Look up a commodity price point in live mode (enforcing `INV-013` freshness).
    pub fn price(&self, commodity: Commodity) -> Result<PricePoint, EconError> {
        self.check_fresh()?;
        self.price_historical(commodity)
    }

    /// Look up a commodity price point even on a superseded historical deck (explicit replay).
    pub fn price_historical(&self, commodity: Commodity) -> Result<PricePoint, EconError> {
        self.prices
            .get(&commodity)
            .copied()
            .ok_or(EconError::MissingPrice {
                deck_id: self.deck_id.clone(),
                commodity: commodity.symbol(),
            })
    }
}

/// Audit verdict when checking a quoted per-unit royalty against a stated metal price and rate
/// (`BD-296`, plan §23.4, `FACT E244`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckMixAudit {
    /// Quoted royalty matches `stated_rate × stated_price` within `tolerance`.
    Consistent {
        expected_royalty: Money,
        quoted_royalty: Money,
    },
    /// Quoted royalty does NOT match `stated_rate × stated_price`, exposing a hidden price-deck mix
    /// (e.g. Kibali COG table charging `$85.50/oz = 5.7% × $1,500` inside a `$2,000/oz` table).
    DeckMixFlagged {
        stated_price: Money,
        stated_rate: ExactQty,
        expected_royalty: Money,
        quoted_royalty: Money,
        discrepancy: Money,
        implied_deck_price: Money,
    },
}

/// Audit whether `quoted_royalty` is consistent with `stated_rate × stated_price` (`FACT E244`).
pub fn audit_royalty_deck_mix(
    stated_price: Money,
    stated_rate: ExactQty,
    quoted_royalty: Money,
    tolerance: Money,
) -> Result<DeckMixAudit, EconError> {
    if stated_price.ccy != quoted_royalty.ccy {
        return Err(EconError::CurrencyMismatch {
            left: stated_price.ccy,
            right: quoted_royalty.ccy,
        });
    }
    if stated_price.ccy != tolerance.ccy {
        return Err(EconError::CurrencyMismatch {
            left: stated_price.ccy,
            right: tolerance.ccy,
        });
    }
    if stated_price.minor <= 0
        || stated_rate.signum() <= 0
        || stated_rate.checked_cmp(ExactQty::from_integer(1))? == core::cmp::Ordering::Greater
        || quoted_royalty.minor < 0
        || tolerance.minor < 0
    {
        return Err(EconError::InvalidDomain {
            field: "deck_mix_audit",
            detail: "stated_price > 0, stated_rate in (0, 1], quoted_royalty >= 0, and tolerance >= 0 required".to_string(),
        });
    }
    let expected = money_mul_exact(stated_price, stated_rate)?;
    let diff = quoted_royalty.checked_sub(expected)?;
    if diff.minor.abs() <= tolerance.minor {
        return Ok(DeckMixAudit::Consistent {
            expected_royalty: expected,
            quoted_royalty,
        });
    }
    let inv_rate = ExactQty::from_integer(1).checked_div(stated_rate)?;
    let implied_price = money_mul_exact(quoted_royalty, inv_rate)?;
    Ok(DeckMixAudit::DeckMixFlagged {
        stated_price,
        stated_rate,
        expected_royalty: expected,
        quoted_royalty,
        discrepancy: diff,
        implied_deck_price: implied_price,
    })
}
