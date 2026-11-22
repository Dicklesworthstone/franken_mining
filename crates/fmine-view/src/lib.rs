//! Determinism: BitwisePortable
//! Adit U-layer read-only 3-D scene export (glTF 2.0 / GLB 2.0 subset) and static HTML viewer
//! (`BD-256`, plan §28.3, `INV-002`, `G-CRS-1`). No authority or control write path exists.

pub mod gltf;
pub mod viewer;
pub mod voxel_mesh;

use core::fmt;

use fmine_crs::CrsId;
use fmine_evidence::{Refusal, RefusalCode};

/// Mandatory `no_claim` line embedded in every exported 3-D scene and viewer page.
pub const VIEW_NO_CLAIM: &str = "Read-only 3-D visualization export from FrankenMining. \
Not a resource/reserve disclosure, geotechnical sign-off, regulatory compliance assertion, or \
control authorization (INV-002, INV-011).";

/// Typed errors across `fmine-view` scene extraction and glTF/GLB export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// `G-CRS-1`: scene layers declare incompatible coordinate reference systems.
    CrsMismatch { expected: CrsId, got: CrsId },
    /// Grid or scene contains no active geometry to export.
    EmptyScene { detail: String },
    /// Non-finite or non-positive geometry parameters, invalid color channel, or empty metadata.
    InvalidGeometry { detail: String },
    /// Index, byte offset, or coordinate range overflow.
    Overflow,
}

impl ViewError {
    /// Map every view error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::CrsMismatch { .. } => Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                self.to_string(),
            ),
            Self::EmptyScene { .. } => {
                Refusal::new(RefusalCode::NoClaimInDomain201, self.to_string())
            }
            Self::InvalidGeometry { .. } | Self::Overflow => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
        }
    }
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrsMismatch { expected, got } => {
                write!(
                    f,
                    "G-CRS-1 scene CRS mismatch: expected {expected:?}, got {got:?}"
                )
            }
            Self::EmptyScene { detail } => write!(f, "empty 3-D scene: {detail}"),
            Self::InvalidGeometry { detail } => write!(f, "invalid 3-D geometry: {detail}"),
            Self::Overflow => write!(f, "3-D scene buffer/index overflow"),
        }
    }
}

impl std::error::Error for ViewError {}
