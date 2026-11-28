//! Strata variography (`BD-237`, plan §13 and WP-16): experimental variograms of drill composites, nested
//! anisotropic variogram models, and the weighted least-squares fit of a model to the experimental values.
//!
//! Determinism: BitwisePortable (`+ − × ÷`, `sqrt` and `floor`; `exp`, `ln` and `sin_cos` through `fmine_core::det`;
//! every sum runs in a fixed order).
//!
//! - [`samples`]: points and values of one support in one CRS, from composites on desurveyed traces (`INV-014`).
//! - [`experimental`]: directional experimental semivariograms with lag, angle and bandwidth tolerances.
//! - [`model`]: a nugget plus spherical, exponential and Gaussian structures, each with a GSLIB rotation.
//! - [`fit`]: the fit. Sills are exact non-negative least squares; ranges come from Nelder–Mead over their
//!   logarithms.
#![forbid(unsafe_code)]

pub mod error;
pub mod experimental;
pub mod fit;
pub mod model;
pub mod samples;

pub use error::GeostatError;
