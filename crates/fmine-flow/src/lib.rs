//! Determinism: BitwisePortable
//! Touchstone K1 material-flow graph, stockpiles, and exact extensive conservation laws (`BD-146`,
//! plan §20.1–§20.2, `INV-005`, `PO-BED-6`, `G-TOUCH-3`).

pub mod balance;
pub mod ffactors;
pub mod graph;

use core::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::UnitsError;

/// Typed errors across `fmine-flow` graph construction and exact conservation checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    /// Duplicate node or stream identifier.
    DuplicateId { kind: &'static str, id: String },
    /// Empty, invalid, or structurally incoherent node/stream declaration.
    InvalidGraph { detail: String },
    /// Stream or observation references a node not present in the graph.
    UnknownNode { node_id: String },
    /// Observation references a stream not present in the graph.
    UnknownStream { stream_id: String },
    /// `G-TOUCH-3`: naive mined÷milled ratio attempted across an inventory-holding stockpile/pad.
    StockpileDecouplingViolation {
        source_node: String,
        target_node: String,
        intervening_stockpiles: Vec<String>,
    },
    /// `INV-005` / `PO-BED-6`: exact extensive conservation violated beyond tolerance (never plugged).
    ConservationViolation {
        node_id: String,
        channel: String,
        residual: String,
        tolerance: String,
    },
    /// Negative physical extensive quantity or invalid denominator for derived grade.
    InvalidQuantity { detail: String },
    /// Exact rational arithmetic overflow (`UnitsError::Overflow`).
    Overflow,
}

impl FlowError {
    /// Maps any flow error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::ConservationViolation { .. } => {
                Refusal::new(RefusalCode::Conservation101, self.to_string())
            }
            Self::StockpileDecouplingViolation { .. } => {
                Refusal::new(RefusalCode::SupportMismatch102, self.to_string())
            }
            Self::InvalidQuantity { .. } | Self::Overflow => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::DuplicateId { .. }
            | Self::InvalidGraph { .. }
            | Self::UnknownNode { .. }
            | Self::UnknownStream { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
        }
    }
}

impl From<UnitsError> for FlowError {
    fn from(value: UnitsError) -> Self {
        match value {
            UnitsError::Overflow => Self::Overflow,
            other => Self::InvalidQuantity {
                detail: other.to_string(),
            },
        }
    }
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId { kind, id } => write!(f, "duplicate {kind} id '{id}'"),
            Self::InvalidGraph { detail } => write!(f, "invalid flow graph: {detail}"),
            Self::UnknownNode { node_id } => write!(f, "unknown flow node '{node_id}'"),
            Self::UnknownStream { stream_id } => write!(f, "unknown flow stream '{stream_id}'"),
            Self::StockpileDecouplingViolation {
                source_node,
                target_node,
                intervening_stockpiles,
            } => write!(
                f,
                "G-TOUCH-3: direct period ratio between '{source_node}' and '{target_node}' is \
                 inadmissible across intervening stockpile(s) [{}]",
                intervening_stockpiles.join(", ")
            ),
            Self::ConservationViolation {
                node_id,
                channel,
                residual,
                tolerance,
            } => write!(
                f,
                "INV-005 conservation refused at node '{node_id}' channel '{channel}': \
                 |residual|={residual} > tolerance={tolerance}"
            ),
            Self::InvalidQuantity { detail } => write!(f, "invalid flow quantity: {detail}"),
            Self::Overflow => write!(f, "exact rational overflow in flow balance"),
        }
    }
}

impl std::error::Error for FlowError {}
