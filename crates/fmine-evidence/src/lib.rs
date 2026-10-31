//! Evidence classes, knowledge states, colours (`§7`) and the structured refusal catalog
//! (`FMINE-REF-*`, `BD-037`–`BD-039`, BD-328).

pub mod color;
pub mod epistemic;
pub mod lattice;
mod refusal_catalog;

pub use color::Color;
pub use epistemic::{
    AoaStatus, AssertionSource, EpistemicStatus, EvidenceClass, HypothesisSetId, KnowledgeState,
    KuKind,
};
pub use lattice::project_color;
pub use refusal_catalog::{CATALOG, RefusalCode};

use std::fmt;

/// Structured refusal diagnostic with actionable remedy (`INV-011`, `BD-039`).
///
/// Outside this crate the only constructor is [`Refusal::new`], which takes a [`RefusalCode`], so every
/// refusal carries a code from `registries/refusals.toml` (BD-328):
///
/// ```
/// use fmine_evidence::{Refusal, RefusalCode};
/// let r = Refusal::new(RefusalCode::Conservation101, "metal balance open by 2 t");
/// assert_eq!((r.code, r.exit_code), ("FMINE-REF-101", 3));
/// ```
///
/// A free string is not a code:
///
/// ```compile_fail
/// let _ = fmine_evidence::Refusal::new("FMINE-REF-101", "detail");
/// ```
///
/// and the fields cannot be assembled by hand:
///
/// ```compile_fail
/// let _ = fmine_evidence::Refusal { code: "FMINE-REF-101", exit_code: 3, what: String::new(), fix: "" };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Refusal {
    /// The registered code, `FMINE-REF-nnn`.
    pub code: &'static str,
    /// The CLI exit code (plan §30.1's family rule; `3..=8`).
    pub exit_code: u8,
    /// The code's summary, then `: ` and the specific detail.
    pub what: String,
    /// Concrete remediation step required to resolve the refusal.
    pub fix: &'static str,
}

impl Refusal {
    /// A refusal under the registered `code`, with `detail` appended to the code's summary.
    #[must_use]
    pub fn new(code: RefusalCode, detail: impl Into<String>) -> Self {
        let spec = code.spec();
        let d = detail.into();
        let what = if d.is_empty() {
            spec.summary.to_string()
        } else {
            format!("{}: {}", spec.summary, d)
        };
        Self {
            code: spec.code,
            exit_code: spec.exit_code,
            what,
            fix: spec.fix,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] (exit {}): {} | Fix: {}",
            self.code, self.exit_code, self.what, self.fix
        )
    }
}

impl std::error::Error for Refusal {}

/// A registry row's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalStatus {
    /// A plan §8.4 code.
    Active,
    /// Awaiting the owner's plan amendment (`docs/adr/ADR-REFUSALS-01.md`).
    Proposed,
    /// Held, not yet used.
    Reserved,
}

impl RefusalStatus {
    /// The registry's spelling: `active`, `proposed` or `reserved`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Proposed => "proposed",
            Self::Reserved => "reserved",
        }
    }
}

/// One row of `registries/refusals.toml`, as compiled into [`CATALOG`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusalSpec {
    pub code: &'static str,
    pub key: &'static str,
    pub exit_code: u8,
    pub status: RefusalStatus,
    pub summary: &'static str,
    pub fix: &'static str,
}

impl RefusalCode {
    /// The code as printed, `FMINE-REF-nnn`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.spec().code
    }

    /// The registered code spelled `code`, if there is one. Reading a code never builds a refusal.
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == code)
    }
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_is_the_enum_in_order_and_refusals_carry_its_rows() {
        assert_eq!(CATALOG.len(), RefusalCode::ALL.len());
        let mut prev = "";
        for (spec, code) in CATALOG.iter().zip(RefusalCode::ALL) {
            assert_eq!(code.spec(), spec);
            assert!(
                spec.code.starts_with("FMINE-REF-") && spec.code.len() == 13,
                "{}",
                spec.code
            );
            assert!(spec.code > prev, "{} after {prev}", spec.code);
            prev = spec.code;
            assert!((3..=8).contains(&spec.exit_code), "{}", spec.code);
            assert!(
                !spec.summary.is_empty() && !spec.fix.is_empty(),
                "{}",
                spec.code
            );
            assert_eq!(RefusalCode::parse(spec.code), Some(code));
            assert_eq!(code.to_string(), spec.code);

            let r = Refusal::new(code, "test context");
            assert_eq!(
                (r.code, r.exit_code, r.fix),
                (spec.code, spec.exit_code, spec.fix)
            );
            assert_eq!(r.what, format!("{}: test context", spec.summary));
            assert_eq!(Refusal::new(code, "").what, spec.summary);
        }
        // There is no fallback code: an unregistered spelling parses to nothing.
        assert_eq!(RefusalCode::parse(concat!("FMINE-REF-", "000")), None);
        assert_eq!(RefusalCode::parse("FMINE-REF-1xx"), None);
    }

    #[test]
    fn test_plan_meanings_on_known_codes() {
        // Spot checks that the catalogue carries plan §8.4's meanings, not the pre-BD-328 ones.
        let s = |c: RefusalCode| (c.as_str(), c.spec().exit_code, c.spec().status);
        assert_eq!(
            s(RefusalCode::UnknownReactiveGround401),
            ("FMINE-REF-401", 7, RefusalStatus::Active)
        );
        assert_eq!(
            s(RefusalCode::NonIdentifiableAttribution104),
            ("FMINE-REF-104", 3, RefusalStatus::Active)
        );
        assert_eq!(
            s(RefusalCode::InputMissingOrUnreadable506),
            ("FMINE-REF-506", 5, RefusalStatus::Proposed)
        );
        assert_eq!(
            s(RefusalCode::DigestMismatch505),
            ("FMINE-REF-505", 8, RefusalStatus::Proposed)
        );
        assert!(
            !RefusalCode::BasisMismatch105
                .spec()
                .fix
                .to_lowercase()
                .contains("clamp")
        );
    }
}
