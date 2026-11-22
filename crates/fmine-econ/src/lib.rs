//! Determinism: BitwisePortable
//! Helm K1 versioned price decks, exact-`Money` NSR, price-contingent royalties and taxes, and
//! `CountryRegime` rows (`BD-228`, `BD-296`, plan §15.4, §23.4, `INV-013`).
//!
//! All physical and monetary quantities use `fmine_units::ExactQty` and `fmine_units::Money`
//! (zero bare `f64` parameters in public signatures; L-6).

pub mod fiscal;
pub mod npv;
pub mod price_deck;
pub mod regime;

use core::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{Currency, ExactQty, Money, UnitsError};

/// Errors raised by `fmine-econ`, mapped to registered `FMINE-REF-*` codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EconError {
    /// Superseded price deck or country regime evaluated in live mode (`INV-013`, `FMINE-REF-502`).
    StaleVersion {
        kind: &'static str,
        id: String,
        superseded_by: String,
    },
    /// Cross-currency operation without explicit FX (`BD-026`, `FMINE-REF-105`).
    CurrencyMismatch { left: Currency, right: Currency },
    /// Incompatible price or quantity unit (`FMINE-REF-105`).
    UnitMismatch { detail: String },
    /// Requested commodity price absent from price deck (`FMINE-REF-201`).
    MissingPrice {
        deck_id: String,
        commodity: &'static str,
    },
    /// Asset or period is in `NOT_PUBLISHED` state (`RISK-26`, `FMINE-REF-201`).
    NotPublished { regime_id: String, reason: String },
    /// Parameter outside physical/fiscal domain (`FMINE-REF-109`).
    InvalidDomain { field: &'static str, detail: String },
    /// Exact `i128` arithmetic overflow (`FMINE-REF-109`).
    Overflow,
    /// Malformed registry TOML or duplicate row id (`FMINE-REF-107`).
    RegistryParse { detail: String },
    /// Discounting convention or multi-root IRR is not uniquely identifiable (`FMINE-REF-104`).
    NonIdentifiableConvention { detail: String },
    /// Root finder refused (no sign change, non-finite, or non-convergence; `FMINE-REF-207`).
    NumericalRootFailed { detail: String },
    /// After-tax cash-flow replay requested without a tax schedule (`FMINE-REF-107`).
    MissingTaxSchedule { detail: String },
}

impl EconError {
    /// Convert to a structured registered `Refusal`.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::StaleVersion { .. } => {
                Refusal::new(RefusalCode::StaleAnchor502, self.to_string())
            }
            Self::CurrencyMismatch { .. } | Self::UnitMismatch { .. } => {
                Refusal::new(RefusalCode::BasisMismatch105, self.to_string())
            }
            Self::MissingPrice { .. } | Self::NotPublished { .. } => {
                Refusal::new(RefusalCode::NoClaimInDomain201, self.to_string())
            }
            Self::InvalidDomain { .. } | Self::Overflow => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::RegistryParse { .. } | Self::MissingTaxSchedule { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::NonIdentifiableConvention { .. } => {
                Refusal::new(RefusalCode::NonIdentifiableAttribution104, self.to_string())
            }
            Self::NumericalRootFailed { .. } => {
                Refusal::new(RefusalCode::NumericalCertificateFailed207, self.to_string())
            }
        }
    }
}

impl From<UnitsError> for EconError {
    fn from(err: UnitsError) -> Self {
        match err {
            UnitsError::CurrencyMismatch { left, right } => Self::CurrencyMismatch { left, right },
            UnitsError::Overflow => Self::Overflow,
            other => Self::InvalidDomain {
                field: "units",
                detail: other.to_string(),
            },
        }
    }
}

impl fmt::Display for EconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleVersion {
                kind,
                id,
                superseded_by,
            } => write!(
                f,
                "FMINE-REF-502: {kind} '{id}' was superseded by '{superseded_by}' (INV-013)"
            ),
            Self::CurrencyMismatch { left, right } => {
                write!(
                    f,
                    "FMINE-REF-105: currency mismatch ({left:?} vs {right:?})"
                )
            }
            Self::UnitMismatch { detail } => write!(f, "FMINE-REF-105: unit mismatch: {detail}"),
            Self::MissingPrice { deck_id, commodity } => write!(
                f,
                "FMINE-REF-201: price deck '{deck_id}' has no price for {commodity}"
            ),
            Self::NotPublished { regime_id, reason } => write!(
                f,
                "FMINE-REF-201: regime '{regime_id}' is NOT_PUBLISHED ({reason})"
            ),
            Self::InvalidDomain { field, detail } => {
                write!(f, "FMINE-REF-109: invalid {field}: {detail}")
            }
            Self::Overflow => write!(f, "FMINE-REF-109: exact arithmetic overflow"),
            Self::RegistryParse { detail } => write!(f, "FMINE-REF-107: registry parse: {detail}"),
            Self::NonIdentifiableConvention { detail } => write!(f, "FMINE-REF-104: {detail}"),
            Self::NumericalRootFailed { detail } => write!(f, "FMINE-REF-207: {detail}"),
            Self::MissingTaxSchedule { detail } => write!(f, "FMINE-REF-107: {detail}"),
        }
    }
}

impl std::error::Error for EconError {}

/// Multiply exact `Money` by an exact rational `ExactQty` with deterministic half-away-from-zero
/// rounding to the nearest `10^-4` minor unit.
pub(crate) fn money_mul_exact(money: Money, factor: ExactQty) -> Result<Money, EconError> {
    let prod = ExactQty::from_integer(money.minor).checked_mul(factor)?;
    let num = prod.num();
    let den = prod.den();
    let q = num / den;
    let r = num % den;
    let rounded = if r == 0 {
        q
    } else {
        let twice_abs_r = r.unsigned_abs().checked_mul(2).ok_or(EconError::Overflow)?;
        if twice_abs_r >= den.unsigned_abs() {
            if num >= 0 {
                q.checked_add(1).ok_or(EconError::Overflow)?
            } else {
                q.checked_sub(1).ok_or(EconError::Overflow)?
            }
        } else {
            q
        }
    };
    Ok(Money::from_minor(money.ccy, rounded))
}
