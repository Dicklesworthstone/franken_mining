//! Determinism: BitwisePortable
//!
//! Node composition crate for `fmined` (`BD-119`, plan §26.6–§26.7, §30.3):
//! role-specific `AppSpec` supervisor trees, single-writer cell leases with monotonic fencing
//! epochs (`PO-BED-1`), read-only degraded mode (exit `4`) on writer escalation, and bounded local
//! Unix domain socket IPC (`<node>/fmined.sock`).

pub mod config;
pub mod node;
#[cfg(unix)]
pub mod socket;

use std::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::l0::L0Error;
use fmine_object::cas::CasError;
use fmine_runtime::supervise::{Refused as SupervisorRefused, SpecError};

/// Typed errors across `fmine-service` configuration, supervision, leasing, and socket IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    InvalidConfig {
        detail: String,
    },
    Spec(SpecError),
    Supervisor(SupervisorRefused),
    /// Node entered read-only degraded mode after `failed_cell`'s ledger writer escalated (exit `4`).
    ReadOnlyDegraded {
        failed_cell: String,
    },
    /// Node was shut down; no further operations are admitted.
    Stopped,
    /// Underlying L0/L1 ledger error (including `PO-BED-1` stale lease/epoch fencing).
    Ledger(L0Error),
    /// Underlying `FMT-SEG-1` segment persistence error.
    Segment {
        cell_id: String,
        detail: String,
    },
    /// Underlying `FMT-CAS-1` object store error.
    Cas(CasError),
    /// Socket path is already served by a live `fmined` process.
    SocketInUse {
        path: String,
    },
    /// Bounded IPC request violated schema or size limit.
    Protocol {
        detail: String,
    },
    /// Local filesystem or Unix socket I/O error.
    Io {
        detail: String,
    },
}

impl ServiceError {
    /// Canonical CLI/IPC exit code matching plan §30.1 (including exit `4` for read-only degraded mode).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::ReadOnlyDegraded { .. } => 4,
            Self::Supervisor(SupervisorRefused::ReadOnly { .. }) => 4,
            _ => self.to_refusal().exit_code,
        }
    }

    /// Maps any service error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::InvalidConfig { .. } | Self::Spec(_) => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::ReadOnlyDegraded { .. }
            | Self::Stopped
            | Self::Supervisor(
                SupervisorRefused::ReadOnly { .. }
                | SupervisorRefused::Stopped
                | SupervisorRefused::TimeWentBack,
            ) => Refusal::new(RefusalCode::StaleAnchor502, self.to_string()),
            Self::Supervisor(_) => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::Ledger(e) => e.to_refusal(),
            Self::Segment { .. } | Self::SocketInUse { .. } | Self::Io { .. } => {
                Refusal::new(RefusalCode::InputMissingOrUnreadable506, self.to_string())
            }
            Self::Cas(e) => e.to_refusal(),
            Self::Protocol { .. } => {
                Refusal::new(RefusalCode::SourceCellUnusable208, self.to_string())
            }
        }
    }
}

impl From<SpecError> for ServiceError {
    fn from(value: SpecError) -> Self {
        Self::Spec(value)
    }
}

impl From<SupervisorRefused> for ServiceError {
    fn from(value: SupervisorRefused) -> Self {
        match value {
            SupervisorRefused::ReadOnly { cell } => Self::ReadOnlyDegraded { failed_cell: cell },
            SupervisorRefused::Stopped => Self::Stopped,
            other => Self::Supervisor(other),
        }
    }
}

impl From<L0Error> for ServiceError {
    fn from(value: L0Error) -> Self {
        Self::Ledger(value)
    }
}

impl From<CasError> for ServiceError {
    fn from(value: CasError) -> Self {
        Self::Cas(value)
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { detail } => write!(f, "invalid fmined config: {detail}"),
            Self::Spec(e) => write!(f, "AppSpec compile error: {e:?}"),
            Self::Supervisor(e) => write!(f, "supervisor refused operation: {e:?}"),
            Self::ReadOnlyDegraded { failed_cell } => write!(
                f,
                "node is in read-only degraded mode after writer escalation on '{failed_cell}' (exit 4); run fmine doctor before restarting"
            ),
            Self::Stopped => write!(f, "fmined node has shut down"),
            Self::Ledger(e) => write!(f, "ledger error: {e}"),
            Self::Segment { cell_id, detail } => {
                write!(f, "segment persistence error for '{cell_id}': {detail}")
            }
            Self::Cas(e) => write!(f, "CAS store error: {e}"),
            Self::SocketInUse { path } => {
                write!(f, "socket '{path}' is already served by a live fmined")
            }
            Self::Protocol { detail } => write!(f, "fmined IPC protocol error: {detail}"),
            Self::Io { detail } => write!(f, "fmined I/O error: {detail}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// Escapes a Rust string for deterministic JSON output.
#[must_use]
pub(crate) fn escape_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}
