//! Phantom-typed physical quantities, moisture/recovery bases, exact statutory conversions,
//! `Money`, chemical basis tags, and hazardous byproduct guards (`WP-01`).

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Sub};

/// Errors raised by unit, basis, fraction, currency, or chemical-basis guards.
#[derive(Debug, Clone, PartialEq)]
pub enum UnitsError {
    /// `FMINE-REF-105`: Moisture fraction must lie in `[0.0, 1.0)`.
    MoistureOutOfBounds(f64),
    /// `FMINE-REF-105`: Mass fraction or recovery must lie in `[0.0, 1.0]`.
    FractionOutOfBounds(f64),
    /// Simplex composition does not sum to `1.0` within tolerance.
    SimplexSumInvalid(f64),
    /// `FMINE-REF-107`: Chemical species basis mismatch without stoichiometric conversion.
    ChemicalBasisMismatch { left: ChemBasis, right: ChemBasis },
    /// Currency mismatch in `Money` arithmetic.
    CurrencyMismatch { left: Currency, right: Currency },
    /// Cannot compute mass-weighted grade when total dry mass is `<= 0`.
    ZeroMassWeight,
    /// Arithmetic overflow in `ExactQty` or `Money`.
    Overflow,
}

impl UnitsError {
    /// Returns the canonical `FMINE-REF-*` refusal code if mapped.
    #[must_use]
    pub const fn refusal_code(&self) -> Option<&'static str> {
        match self {
            Self::MoistureOutOfBounds(_) | Self::FractionOutOfBounds(_) => Some("FMINE-REF-105"),
            Self::ChemicalBasisMismatch { .. } => Some("FMINE-REF-107"),
            _ => None,
        }
    }
}

impl fmt::Display for UnitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MoistureOutOfBounds(v) => {
                write!(f, "FMINE-REF-105: moisture fraction {v} outside [0.0, 1.0)")
            }
            Self::FractionOutOfBounds(v) => {
                write!(f, "FMINE-REF-105: fraction {v} outside [0.0, 1.0]")
            }
            Self::SimplexSumInvalid(s) => write!(f, "simplex sum {s} != 1.0"),
            Self::ChemicalBasisMismatch { left, right } => write!(
                f,
                "FMINE-REF-107: chemical basis mismatch ({left:?} vs {right:?})"
            ),
            Self::CurrencyMismatch { left, right } => {
                write!(f, "currency mismatch ({left:?} vs {right:?})")
            }
            Self::ZeroMassWeight => write!(f, "zero total dry mass in weighted mean"),
            Self::Overflow => write!(f, "exact arithmetic overflow"),
        }
    }
}

impl std::error::Error for UnitsError {}

// --- Phantom Dimension, MassBasis, and MetalStage markers (BD-023) ---

/// Marker for mass dimension (tonnes, grams, troy ounces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mass;
/// Marker for length dimension (metres).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Length;
/// Marker for volume dimension (cubic metres).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volume;
/// Marker for energy dimension (kilowatt-hours, gigajoules).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Energy;

/// Wet mass basis (includes pore moisture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wet;
/// Dry mass basis (moisture removed; canonical denominator for ore grades).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dry;
/// In-situ geological basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InSitu;
/// Dimensionless / basis-neutral marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Neutral;

/// Contained metal in ore before metallurgical recovery loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contained;
/// Recoverable metal after applying process recovery factor (`INV-012`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recoverable;
/// Produced metal at plant / doré / concentrate output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Produced;
/// Payable metal after smelter/refinery payability deductions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payable;
/// Sold metal recognized in financial period sales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sold;

/// Phantom-typed physical quantity `Qty<D, B, R>`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Qty<D, B = Neutral, R = Neutral> {
    value: f64,
    _marker: PhantomData<(D, B, R)>,
}

impl<D, B, R> Qty<D, B, R> {
    /// Creates a new phantom-typed quantity.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying scalar value in canonical SI/mining units.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }
}

impl<D: Copy, B: Copy, R: Copy> Add for Qty<D, B, R> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<D: Copy, B: Copy, R: Copy> Sub for Qty<D, B, R> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

/// Validated moisture fraction in `[0.0, 1.0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoistureFraction(f64);

impl MoistureFraction {
    /// Constructs a validated moisture fraction in `[0.0, 1.0)`.
    pub fn new(v: f64) -> Result<Self, UnitsError> {
        if !v.is_finite() || !(0.0..1.0).contains(&v) {
            return Err(UnitsError::MoistureOutOfBounds(v));
        }
        Ok(Self(v))
    }

    /// Returns the scalar moisture fraction.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Validated mass fraction or metallurgical recovery in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassFraction(f64);

impl MassFraction {
    /// Constructs a validated mass fraction in `[0.0, 1.0]`.
    pub fn new(v: f64) -> Result<Self, UnitsError> {
        if !v.is_finite() || !(0.0..=1.0).contains(&v) {
            return Err(UnitsError::FractionOutOfBounds(v));
        }
        Ok(Self(v))
    }

    /// Returns the scalar mass fraction.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Converts wet tonnes to dry tonnes: `dry = wet * (1 - moisture)`.
#[must_use]
pub fn wet_to_dry(wet_tonnes: Qty<Mass, Wet, Neutral>, moisture: MoistureFraction) -> Qty<Mass, Dry, Neutral> {
    Qty::new(wet_tonnes.value() * (1.0 - moisture.value()))
}

/// Ore grade (intensive quantity, e.g., g/t dry or mass fraction).
///
/// Intentionally does **not** implement `std::ops::Add` (`INV-012`): grades are non-additive
/// without dry-mass weighting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grade<R = Contained> {
    grams_per_dry_tonne: f64,
    _stage: PhantomData<R>,
}

impl<R> Grade<R> {
    /// Creates a grade in grams per dry metric tonne (`g/t`).
    #[must_use]
    pub const fn from_g_per_t(grams_per_dry_tonne: f64) -> Self {
        Self {
            grams_per_dry_tonne,
            _stage: PhantomData,
        }
    }

    /// Returns the grade in grams per dry metric tonne (`g/t`).
    #[must_use]
    pub const fn g_per_t(self) -> f64 {
        self.grams_per_dry_tonne
    }
}

impl Grade<Contained> {
    /// Applies a validated metallurgical recovery factor to convert `Grade<Contained>`
    /// into `Grade<Recoverable>`.
    #[must_use]
    pub fn apply_recovery(self, recovery: MassFraction) -> Grade<Recoverable> {
        Grade::from_g_per_t(self.grams_per_dry_tonne * recovery.value())
    }
}

/// Computes the mass-weighted mean grade across `(dry_tonnes, grade)` pairs (`INV-012`).
pub fn mass_weighted_mean<R: Copy>(
    samples: &[(Qty<Mass, Dry, Neutral>, Grade<R>)],
) -> Result<(Qty<Mass, Dry, Neutral>, Grade<R>), UnitsError> {
    let mut total_mass = 0.0_f64;
    let mut total_metal_grams = 0.0_f64;
    for &(mass, grade) in samples {
        if mass.value() < 0.0 || !mass.value().is_finite() || !grade.g_per_t().is_finite() {
            return Err(UnitsError::ZeroMassWeight);
        }
        total_mass += mass.value();
        total_metal_grams += mass.value() * grade.g_per_t();
    }
    if total_mass <= 0.0 {
        return Err(UnitsError::ZeroMassWeight);
    }
    Ok((
        Qty::new(total_mass),
        Grade::from_g_per_t(total_metal_grams / total_mass),
    ))
}

/// Computes contained or recoverable metal (in grams) from dry ore tonnes and grade.
#[must_use]
pub fn metal_grams<R>(dry_tonnes: Qty<Mass, Dry, Neutral>, grade: Grade<R>) -> Qty<Mass, Dry, R> {
    Qty::new(dry_tonnes.value() * grade.g_per_t())
}

// --- Exact Statutory Conversions (BD-024, INV-006) ---

/// Exact statutory numerator for troy ounce to grams: `31.1034768 g = 311_034_768 / 10_000_000 g`.
pub const TROY_OZ_TO_GRAMS_NUM: i128 = 311_034_768;
/// Exact statutory denominator for troy ounce to grams.
pub const TROY_OZ_TO_GRAMS_DEN: i128 = 10_000_000;

/// Exact statutory numerator for avoirdupois pound to grams: `453.59237 g = 45_359_237 / 100_000 g`.
pub const AVDP_LB_TO_GRAMS_NUM: i128 = 45_359_237;
/// Exact statutory denominator for avoirdupois pound to grams.
pub const AVDP_LB_TO_GRAMS_DEN: i128 = 100_000;

/// Exact statutory ratio for `oz t / short ton` to `g / metric tonne`: `240 / 7 = 34.285714...`.
pub const OZ_PER_ST_TO_G_PER_T_NUM: i128 = 240;
/// Exact statutory denominator for `oz t / short ton` to `g / metric tonne`.
pub const OZ_PER_ST_TO_G_PER_T_DEN: i128 = 7;

/// Exact rational quantity `num / den` (`den > 0`) for zero-loss statutory conversions (`BD-024`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactQty {
    num: i128,
    den: i128,
}

impl ExactQty {
    /// Creates a reduced exact rational `num / den`.
    pub fn new(num: i128, den: i128) -> Result<Self, UnitsError> {
        if den == 0 {
            return Err(UnitsError::Overflow);
        }
        let (n, d) = if den < 0 { (-num, -den) } else { (num, den) };
        let g = gcd_i128(n.unsigned_abs(), d.unsigned_abs()) as i128;
        Ok(Self {
            num: n / g,
            den: d / g,
        })
    }

    /// Creates an exact integer quantity `n / 1`.
    #[must_use]
    pub const fn from_integer(n: i128) -> Self {
        Self { num: n, den: 1 }
    }

    /// Numerator of the reduced rational.
    #[must_use]
    pub const fn num(self) -> i128 {
        self.num
    }

    /// Denominator of the reduced rational.
    #[must_use]
    pub const fn den(self) -> i128 {
        self.den
    }

    /// Multiplies two `ExactQty` values exactly.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, UnitsError> {
        let g1 = gcd_i128(self.num.unsigned_abs(), rhs.den.unsigned_abs()) as i128;
        let g2 = gcd_i128(rhs.num.unsigned_abs(), self.den.unsigned_abs()) as i128;
        let n = (self.num / g1)
            .checked_mul(rhs.num / g2)
            .ok_or(UnitsError::Overflow)?;
        let d = (self.den / g2)
            .checked_mul(rhs.den / g1)
            .ok_or(UnitsError::Overflow)?;
        Self::new(n, d)
    }

    /// Divides `self` by `rhs` exactly.
    pub fn checked_div(self, rhs: Self) -> Result<Self, UnitsError> {
        if rhs.num == 0 {
            return Err(UnitsError::Overflow);
        }
        self.checked_mul(Self::new(rhs.den, rhs.num)?)
    }

    /// Converts troy ounces to grams using exact statutory `31.1034768 g/oz t`.
    pub fn troy_oz_to_grams(self) -> Result<Self, UnitsError> {
        self.checked_mul(Self {
            num: TROY_OZ_TO_GRAMS_NUM,
            den: TROY_OZ_TO_GRAMS_DEN,
        })
    }

    /// Converts grams to troy ounces using exact statutory `31.1034768 g/oz t`.
    pub fn grams_to_troy_oz(self) -> Result<Self, UnitsError> {
        self.checked_mul(Self {
            num: TROY_OZ_TO_GRAMS_DEN,
            den: TROY_OZ_TO_GRAMS_NUM,
        })
    }

    /// Converts avoirdupois pounds to grams using exact statutory `453.59237 g/lb`.
    pub fn avdp_lb_to_grams(self) -> Result<Self, UnitsError> {
        self.checked_mul(Self {
            num: AVDP_LB_TO_GRAMS_NUM,
            den: AVDP_LB_TO_GRAMS_DEN,
        })
    }

    /// Converts grams to avoirdupois pounds using exact statutory `453.59237 g/lb`.
    pub fn grams_to_avdp_lb(self) -> Result<Self, UnitsError> {
        self.checked_mul(Self {
            num: AVDP_LB_TO_GRAMS_DEN,
            den: AVDP_LB_TO_GRAMS_NUM,
        })
    }

    /// Converts `oz t / short ton` to `g / metric tonne` using exact `240 / 7`.
    pub fn oz_per_st_to_g_per_t(self) -> Result<Self, UnitsError> {
        self.checked_mul(Self {
            num: OZ_PER_ST_TO_G_PER_T_NUM,
            den: OZ_PER_ST_TO_G_PER_T_DEN,
        })
    }

    /// Converts `g / metric tonne` to `oz t / short ton` using exact `7 / 240`.
    pub fn g_per_t_to_oz_per_st(self) -> Result<Self, UnitsError> {
        self.checked_mul(Self {
            num: OZ_PER_ST_TO_G_PER_T_DEN,
            den: OZ_PER_ST_TO_G_PER_T_NUM,
        })
    }

    /// Evaluates to `f64` only at presentation boundaries.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        (self.num as f64) / (self.den as f64)
    }
}

const fn gcd_i128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    if a == 0 { 1 } else { a }
}

// --- Money & Currency (BD-025) ---

/// ISO 4217 currency code subset used across mining disclosures and sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Currency {
    USD,
    CAD,
    AUD,
    ZAR,
    PKR,
    CLP,
    PEN,
    ARS,
    TZS,
    XAF,
    CDF,
    ZMW,
    PGK,
    DOP,
    SAR,
    EUR,
    JPY,
}

/// Exact monetary value in `10^-4` currency units (100 minor units per cent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    pub ccy: Currency,
    /// Value in `10^-4` major currency units (e.g. `$1.00 = 10_000`).
    pub minor: i128,
}

impl Money {
    /// Scale factor: `10_000` minor units per `1.0000` major currency unit.
    pub const SCALE: i128 = 10_000;

    /// Constructs `Money` from exact minor units (`10^-4` major units).
    #[must_use]
    pub const fn from_minor(ccy: Currency, minor: i128) -> Self {
        Self { ccy, minor }
    }

    /// Constructs `Money` from whole major units (e.g., USD dollars).
    #[must_use]
    pub const fn from_major(ccy: Currency, major: i128) -> Self {
        Self {
            ccy,
            minor: major * Self::SCALE,
        }
    }

    /// Constructs `Money` from `USD` millions (`$M`), where `1.0 $M = 10_000_000_000` minor units.
    #[must_use]
    pub fn from_millions_f64(ccy: Currency, millions: f64) -> Self {
        let minor = (millions * 1.0e10_f64).round() as i128;
        Self { ccy, minor }
    }

    /// Checked addition requiring identical currencies.
    pub fn checked_add(self, rhs: Self) -> Result<Self, UnitsError> {
        if self.ccy != rhs.ccy {
            return Err(UnitsError::CurrencyMismatch {
                left: self.ccy,
                right: rhs.ccy,
            });
        }
        Ok(Self {
            ccy: self.ccy,
            minor: self.minor.checked_add(rhs.minor).ok_or(UnitsError::Overflow)?,
        })
    }

    /// Checked subtraction requiring identical currencies.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, UnitsError> {
        if self.ccy != rhs.ccy {
            return Err(UnitsError::CurrencyMismatch {
                left: self.ccy,
                right: rhs.ccy,
            });
        }
        Ok(Self {
            ccy: self.ccy,
            minor: self.minor.checked_sub(rhs.minor).ok_or(UnitsError::Overflow)?,
        })
    }

    /// Returns the major currency amount as `f64` for reporting.
    #[must_use]
    pub fn to_major_f64(self) -> f64 {
        (self.minor as f64) / (Self::SCALE as f64)
    }

    /// Allocates `self` across `weights` using deterministic largest-remainder rounding
    /// so that the sum of allocated slices equals `self.minor` with `0` residual (`BD-025`).
    pub fn allocate_pro_rata(self, weights: &[u64]) -> Result<Vec<Self>, UnitsError> {
        let total_weight: u128 = weights.iter().map(|&w| u128::from(w)).sum();
        if total_weight == 0 {
            return Err(UnitsError::ZeroMassWeight);
        }
        let sign = if self.minor < 0 { -1_i128 } else { 1_i128 };
        let abs_minor = self.minor.unsigned_abs();
        let mut floors = Vec::with_capacity(weights.len());
        let mut remainders = Vec::with_capacity(weights.len());
        let mut allocated: u128 = 0;

        for (idx, &w) in weights.iter().enumerate() {
            let num = abs_minor
                .checked_mul(u128::from(w))
                .ok_or(UnitsError::Overflow)?;
            let q = num / total_weight;
            let r = num % total_weight;
            floors.push(q);
            remainders.push((r, idx));
            allocated += q;
        }

        let shortfall = (abs_minor - allocated) as usize;
        // Sort by descending remainder, then ascending index for deterministic tie-breaking.
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        for &(_, idx) in remainders.iter().take(shortfall) {
            floors[idx] += 1;
        }

        Ok(floors
            .into_iter()
            .map(|q| Self {
                ccy: self.ccy,
                minor: sign * (q as i128),
            })
            .collect())
    }
}

/// Explicit foreign-exchange rate (`1 from = rate_num / rate_den to`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxRate {
    pub from: Currency,
    pub to: Currency,
    pub rate_num: i128,
    pub rate_den: i128,
}

impl FxRate {
    /// Converts `money` from `self.from` into `self.to`.
    pub fn convert(self, money: Money) -> Result<Money, UnitsError> {
        if money.ccy != self.from {
            return Err(UnitsError::CurrencyMismatch {
                left: money.ccy,
                right: self.from,
            });
        }
        if self.rate_den == 0 {
            return Err(UnitsError::Overflow);
        }
        let minor = money
            .minor
            .checked_mul(self.rate_num)
            .ok_or(UnitsError::Overflow)?
            / self.rate_den;
        Ok(Money::from_minor(self.to, minor))
    }
}

// --- Chemical Species Basis Tags (BD-026, INV-012, FMINE-REF-107) ---

/// Explicit chemical reporting basis for environmental and metallurgical concentrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChemBasis {
    /// Nitrate reported as elemental Nitrogen (`mg/L as N`, `MW = 14.0067`).
    ElementalN,
    /// Nitrate reported as the nitrate ion (`mg/L as NO3`, `MW = 62.0049`).
    NitrateNO3,
    /// Weak-acid dissociable cyanide (`mg/L WAD CN`).
    CyanideWAD,
    /// Free cyanide (`mg/L Free CN`).
    CyanideFree,
    /// Total cyanide (`mg/L Total CN`).
    CyanideTotal,
    /// Acidity/alkalinity reported as `CaCO3` equivalent (`kg CaCO3 / t`).
    CaCO3Equivalent,
    /// Filtered (`0.45 um`) dissolved metal concentration.
    Dissolved,
    /// Unfiltered total recoverable metal concentration.
    TotalRecoverable,
}

/// Stoichiometric ratio `MW(NO3) / MW(N) = 62.0049 / 14.0067 = 4.426803...`.
pub const NO3_TO_N_STOICHIOMETRIC_RATIO: f64 = 62.0049 / 14.0067;

/// Concentration tagged with its chemical reporting basis (`mg/L`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChemConc {
    pub mg_per_l: f64,
    pub basis: ChemBasis,
}

impl ChemConc {
    /// Creates a basis-tagged concentration (`mg/L`).
    #[must_use]
    pub const fn new(mg_per_l: f64, basis: ChemBasis) -> Self {
        Self { mg_per_l, basis }
    }

    /// Converts between `ElementalN` and `NitrateNO3` using exact molecular weights.
    pub fn convert_basis(self, target: ChemBasis) -> Result<Self, UnitsError> {
        if self.basis == target {
            return Ok(self);
        }
        match (self.basis, target) {
            (ChemBasis::ElementalN, ChemBasis::NitrateNO3) => Ok(Self {
                mg_per_l: self.mg_per_l * NO3_TO_N_STOICHIOMETRIC_RATIO,
                basis: ChemBasis::NitrateNO3,
            }),
            (ChemBasis::NitrateNO3, ChemBasis::ElementalN) => Ok(Self {
                mg_per_l: self.mg_per_l / NO3_TO_N_STOICHIOMETRIC_RATIO,
                basis: ChemBasis::ElementalN,
            }),
            (left, right) => Err(UnitsError::ChemicalBasisMismatch { left, right }),
        }
    }

    /// Returns `Ok(true)` if `self > threshold` after enforcing identical `ChemBasis`,
    /// or `Err(UnitsError::ChemicalBasisMismatch)` (`FMINE-REF-107`) if bases differ.
    pub fn exceeds_strict(self, threshold: Self) -> Result<bool, UnitsError> {
        if self.basis != threshold.basis {
            return Err(UnitsError::ChemicalBasisMismatch {
                left: self.basis,
                right: threshold.basis,
            });
        }
        Ok(self.mg_per_l > threshold.mg_per_l)
    }
}

/// Simplex vector whose `N` components lie in `[0, 1]` and sum to `1.0` (`BD-027`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimplexVector<const N: usize>([f64; N]);

impl<const N: usize> SimplexVector<N> {
    /// Validates that all components are in `[0, 1]` and sum to `1.0 ± 1e-9`.
    pub fn new(values: [f64; N]) -> Result<Self, UnitsError> {
        let mut sum = 0.0_f64;
        for &v in &values {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err(UnitsError::FractionOutOfBounds(v));
            }
            sum += v;
        }
        if (sum - 1.0).abs() > 1e-9 {
            return Err(UnitsError::SimplexSumInvalid(sum));
        }
        Ok(Self(values))
    }

    /// Returns the validated components.
    #[must_use]
    pub const fn components(&self) -> &[f64; N] {
        &self.0
    }
}

// --- Hazardous Byproduct Non-Monetization Guard (BD-277, G-BYPROD-1) ---

/// Hazardous process byproduct (`Hg`, `As2O3`) that cannot be monetized or credited to AISC.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HazardousByproduct {
    /// Captured elemental mercury (`tonnes Hg`), e.g. Veladero (`61 t` stored in 2024).
    ElementalMercury { tonnes: f64 },
    /// Captured arsenic trioxide / scorodite residue (`tonnes`).
    ArsenicResidue { tonnes: f64 },
}

/// Long-term storage and stewardship liability schedule for a hazardous byproduct (`G-BYPROD-1`).
#[derive(Debug, Clone, PartialEq)]
pub struct StorageLiabilitySchedule {
    pub cumulative_tonnes: f64,
    pub annual_stewardship_cost: Money,
    pub horizon_years: u32,
}

impl HazardousByproduct {
    /// Maps a hazardous byproduct strictly to a storage liability schedule (`G-BYPROD-1`).
    /// No method exists to convert `HazardousByproduct` into positive revenue or AISC credit.
    #[must_use]
    pub fn into_storage_liability(
        self,
        cost_per_tonne_year: Money,
        horizon_years: u32,
    ) -> StorageLiabilitySchedule {
        let tonnes = match self {
            Self::ElementalMercury { tonnes } | Self::ArsenicResidue { tonnes } => tonnes.max(0.0),
        };
        let annual_minor = ((cost_per_tonne_year.minor as f64) * tonnes).round() as i128;
        StorageLiabilitySchedule {
            cumulative_tonnes: tonnes,
            annual_stewardship_cost: Money::from_minor(cost_per_tonne_year.ccy, annual_minor),
            horizon_years,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wet_to_dry_and_contained_metal() {
        let wet = Qty::<Mass, Wet>::new(1_000.0);
        let moisture = MoistureFraction::new(0.08).expect("valid moisture");
        let dry = wet_to_dry(wet, moisture);
        assert!((dry.value() - 920.0).abs() < 1e-12);

        let g1 = Grade::<Contained>::from_g_per_t(2.5);
        let g2 = Grade::<Contained>::from_g_per_t(1.5);
        let (tot_dry, blended) = mass_weighted_mean(&[(dry, g1), (Qty::new(920.0), g2)])
            .expect("valid weighted mean");
        assert!((tot_dry.value() - 1_840.0).abs() < 1e-12);
        assert!((blended.g_per_t() - 2.0).abs() < 1e-12);

        let rec = MassFraction::new(0.89).expect("valid recovery");
        let rec_grade = blended.apply_recovery(rec);
        let rec_grams = metal_grams(tot_dry, rec_grade);
        assert!((rec_grams.value() - 1_840.0 * 2.0 * 0.89).abs() < 1e-9);
    }

    #[test]
    fn test_exact_statutory_conversions_10k_roundtrip() {
        for i in 1..=10_000_i128 {
            let oz = ExactQty::new(i, 100).expect("valid exact qty");
            let g = oz.troy_oz_to_grams().expect("oz -> g");
            let back = g.grams_to_troy_oz().expect("g -> oz");
            assert_eq!(oz, back, "troy oz roundtrip failed at {i}");

            let lb = ExactQty::new(i, 50).expect("valid lb");
            let g_lb = lb.avdp_lb_to_grams().expect("lb -> g");
            let back_lb = g_lb.grams_to_avdp_lb().expect("g -> lb");
            assert_eq!(lb, back_lb, "avoirdupois lb roundtrip failed at {i}");

            let oz_st = ExactQty::new(i, 10).expect("valid oz/st");
            let gpt = oz_st.oz_per_st_to_g_per_t().expect("oz/st -> g/t");
            let back_oz_st = gpt.g_per_t_to_oz_per_st().expect("g/t -> oz/st");
            assert_eq!(oz_st, back_oz_st, "oz/st roundtrip failed at {i}");
        }
    }

    #[test]
    fn test_money_pro_rata_conservation() {
        let total = Money::from_minor(Currency::USD, 1_000_003);
        let weights = [33_u64, 33, 34];
        let parts = total.allocate_pro_rata(&weights).expect("pro-rata");
        let sum: i128 = parts.iter().map(|m| m.minor).sum();
        assert_eq!(sum, total.minor);
    }

    #[test]
    fn test_reko_diq_nitrate_basis_conversion() {
        // Reko Diq ESIA FS13 groundwater sample: 79 mg/L as N = 349.717 mg/L as NO3 > 50 mg/L WHO limit.
        let sample_as_n = ChemConc::new(79.0, ChemBasis::ElementalN);
        let who_limit_no3 = ChemConc::new(50.0, ChemBasis::NitrateNO3);

        // Comparing without conversion must trigger FMINE-REF-107.
        let err = sample_as_n
            .exceeds_strict(who_limit_no3)
            .expect_err("must refuse mixed basis");
        assert_eq!(err.refusal_code(), Some("FMINE-REF-107"));

        // Converting to NitrateNO3 reveals the 7x exceedance.
        let sample_as_no3 = sample_as_n
            .convert_basis(ChemBasis::NitrateNO3)
            .expect("stoichiometric N -> NO3 conversion");
        assert!((sample_as_no3.mg_per_l - 349.7174).abs() < 0.01);
        assert!(sample_as_no3
            .exceeds_strict(who_limit_no3)
            .expect("same basis comparison"));
    }

    #[test]
    fn test_hazardous_byproduct_cannot_monetize() {
        // Veladero 2024 stored mercury: 61 tonnes + 48 tonnes produced.
        let hg = HazardousByproduct::ElementalMercury { tonnes: 109.0 };
        let schedule = hg.into_storage_liability(Money::from_major(Currency::USD, 1_200), 30);
        assert_eq!(schedule.cumulative_tonnes, 109.0);
        assert_eq!(schedule.horizon_years, 30);
        assert!(schedule.annual_stewardship_cost.minor > 0);
    }
}
