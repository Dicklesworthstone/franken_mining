//! Epistemic evidence lattice (`EvidenceClass`, `KnowledgeState`, `ClaimColor`) and
//! structured refusal catalog (`FMINE-REF-*`, `BD-037`–`BD-039`).

pub mod lattice;

pub use lattice::{ClaimColor, EvidenceClass, KnowledgeState, project_color};

use std::fmt;

/// Structured refusal diagnostic with actionable remedy (`INV-011`, `BD-039`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Canonical code (`FMINE-REF-101` .. `FMINE-REF-502`).
    pub code: &'static str,
    ///Deterministic CLI exit code (`2..=8`).
    pub exit_code: u8,
    /// Human- and agent-readable explanation of what guard failed.
    pub what: String,
    /// Concrete remediation step required to resolve the refusal.
    pub fix: &'static str,
}

impl Refusal {
    /// Constructs a `Refusal` from a canonical `FMINE-REF-*` code and specific context detail.
    #[must_use]
    pub fn from_code(code: &'static str, detail: impl Into<String>) -> Self {
        let spec = lookup_refusal_spec(code).unwrap_or(RefusalSpec {
            code: "FMINE-REF-000",
            exit_code: 2,
            summary: "Unspecified invariant refusal",
            fix: "Inspect the refusal detail and consult registries/refusals.toml.",
        });
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

/// Static refusal specification entry matching `registries/refusals.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusalSpec {
    pub code: &'static str,
    pub exit_code: u8,
    pub summary: &'static str,
    pub fix: &'static str,
}

/// Complete static catalog of canonical `FMINE-REF-*` refusal specifications (`BD-039`).
pub const REFUSAL_CATALOG: &[RefusalSpec] = &[
    RefusalSpec {
        code: "FMINE-REF-101",
        exit_code: 3,
        summary: "Coordinate Reference System mismatch across spatial operands",
        fix: "Reproject both geometries to a common CRS via fmine_crs before combining.",
    },
    RefusalSpec {
        code: "FMINE-REF-102",
        exit_code: 3,
        summary: "Change-of-support mismatch (point assay vs SMU block)",
        fix: "Apply discrete Gaussian or affine support correction before comparing point and block grades.",
    },
    RefusalSpec {
        code: "FMINE-REF-103",
        exit_code: 3,
        summary: "Unweighted arithmetic averaging of intensive/non-additive variable",
        fix: "Weight grades by dry mass or convert to extensive metal content before averaging.",
    },
    RefusalSpec {
        code: "FMINE-REF-104",
        exit_code: 3,
        summary: "Density or specific gravity missing in volume-to-mass conversion",
        fix: "Provide a domain-calibrated dry bulk density Claim on the block or composite.",
    },
    RefusalSpec {
        code: "FMINE-REF-105",
        exit_code: 3,
        summary: "Mass fraction, moisture, or recovery outside valid [0, 1] simplex",
        fix: "Clamp or re-normalize compositions on the simplex and verify moisture < 1.0.",
    },
    RefusalSpec {
        code: "FMINE-REF-106",
        exit_code: 3,
        summary: "Mine-grid coordinate extrapolated outside control-monument convex hull",
        fix: "Add bounding survey control monuments or restrict evaluation to the convex hull buffer.",
    },
    RefusalSpec {
        code: "FMINE-REF-107",
        exit_code: 3,
        summary: "Chemical reporting basis mismatch (e.g. NO3-N vs NO3 ion)",
        fix: "Convert concentrations to a common stoichiometric basis via ChemConc::convert_basis.",
    },
    RefusalSpec {
        code: "FMINE-REF-108",
        exit_code: 3,
        summary: "Assay-lag lookahead violation (t_assay_available > as_of_decision)",
        fix: "Filter operational routing inputs to assays reported at or before as_of_decision.",
    },
    RefusalSpec {
        code: "FMINE-REF-201",
        exit_code: 4,
        summary: "Mass or metal balance discrepancy exceeds 3-sigma tolerance without BILLED split",
        fix: "Run fmine reconcile mass-balance to decompose discrepancy or attach an adjustment node.",
    },
    RefusalSpec {
        code: "FMINE-REF-202",
        exit_code: 4,
        summary: "Accounting double-entry journal fails zero-sum balance check",
        fix: "Ensure debits equal credits in minor currency units for every journal entry.",
    },
    RefusalSpec {
        code: "FMINE-REF-203",
        exit_code: 4,
        summary: "Disclosure roll-up mismatch without explicit consolidation adjustment layer",
        fix: "Supply L_corp and L_JV adjustment layers to reconcile site sums to consolidated totals.",
    },
    RefusalSpec {
        code: "FMINE-REF-301",
        exit_code: 5,
        summary: "Kriging or Cholesky covariance matrix is non-positive-definite or singular",
        fix: "Check duplicate sample coordinates or add a positive nugget variance.",
    },
    RefusalSpec {
        code: "FMINE-REF-302",
        exit_code: 5,
        summary: "Geotechnical Factor of Safety or Probability of Failure violates safety envelope",
        fix: "Flatten inter-ramp slope angle or depressurize pore water before scheduling bench.",
    },
    RefusalSpec {
        code: "FMINE-REF-303",
        exit_code: 5,
        summary: "Tailings GISTM trigger action response plan (TARP) red threshold exceeded",
        fix: "Halt deposition into the affected cell and execute emergency drawdown.",
    },
    RefusalSpec {
        code: "FMINE-REF-304",
        exit_code: 5,
        summary: "Schedule optimization LP/MILP is infeasible under capacity and precedence constraints",
        fix: "Inspect the IIS minimal conflicting constraint set in the solver diagnostic.",
    },
    RefusalSpec {
        code: "FMINE-REF-401",
        exit_code: 6,
        summary: "Epistemic gate refuses Green status on Placeholder or failed-calibration claim",
        fix: "Replace placeholder inputs with calibrated claims or downgrade target status.",
    },
    RefusalSpec {
        code: "FMINE-REF-402",
        exit_code: 6,
        summary: "Confidentiality lattice violation: export level below claim sensitivity",
        fix: "Aggregate or redact restricted claims before exporting to a lower clearance tier.",
    },
    RefusalSpec {
        code: "FMINE-REF-403",
        exit_code: 6,
        summary: "Forecast ledger mutation attempted after freeze_time",
        fix: "Create a new timestamped forecast entry rather than mutating a frozen ledger record.",
    },
    RefusalSpec {
        code: "FMINE-REF-404",
        exit_code: 6,
        summary: "Decision card missing required Unknown-Unknown (UuTerm) tail-risk slot",
        fix: "Attach at least one explicit UuTerm scenario or structural residual to the DecisionCard.",
    },
    RefusalSpec {
        code: "FMINE-REF-501",
        exit_code: 7,
        summary: "Mine schedule step violates physical bench-precedence cone",
        fix: "Ensure all overlying predecessor blocks are mined in earlier or equal periods.",
    },
    RefusalSpec {
        code: "FMINE-REF-502",
        exit_code: 8,
        summary: "Artifact SHA-256 or BLAKE3 content digest mismatch against manifest",
        fix: "Re-verify input file integrity or regenerate the bundle from canonical sources.",
    },
    RefusalSpec {
        code: "FMINE-REF-504",
        exit_code: 7,
        summary: "Uncured backfill exposure: secondary stope scheduled adjacent to uncured primary fill (G-HEAD-7)",
        fix: "Delay secondary stope blast until paste-fill hydration reaches required UCS threshold.",
    },
];

/// Looks up a `RefusalSpec` by its canonical `FMINE-REF-*` code.
#[must_use]
pub fn lookup_refusal_spec(code: &str) -> Option<RefusalSpec> {
    REFUSAL_CATALOG.iter().copied().find(|s| s.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_state_and_color_projection() {
        // Placeholder is always Red (NL-4).
        assert_eq!(
            project_color(
                EvidenceClass::Placeholder,
                KnowledgeState::KnownKnown,
                false,
                false,
                false
            ),
            ClaimColor::Red
        );
        // Failed calibration is always Red (NL-8).
        assert_eq!(
            project_color(
                EvidenceClass::Calibrated,
                KnowledgeState::KnownKnown,
                true,
                false,
                false
            ),
            ClaimColor::Red
        );
        // Material UU or stale is at most Amber (NL-5, NL-7).
        assert_eq!(
            project_color(
                EvidenceClass::Exact,
                KnowledgeState::KnownKnown,
                false,
                true,
                false
            ),
            ClaimColor::Amber
        );
        assert_eq!(
            project_color(
                EvidenceClass::Calibrated,
                KnowledgeState::KnownUnknown,
                false,
                false,
                true
            ),
            ClaimColor::Amber
        );
        // Calibrated + KU + not stale + no material UU -> Green.
        assert_eq!(
            project_color(
                EvidenceClass::Calibrated,
                KnowledgeState::KnownUnknown,
                false,
                false,
                false
            ),
            ClaimColor::Green
        );
    }

    #[test]
    fn test_refusal_catalog_completeness() {
        assert_eq!(REFUSAL_CATALOG.len(), 22);
        for spec in REFUSAL_CATALOG {
            assert!(spec.code.starts_with("FMINE-REF-"));
            assert!((3..=8).contains(&spec.exit_code));
            assert!(!spec.summary.is_empty());
            assert!(!spec.fix.is_empty());
            let r = Refusal::from_code(spec.code, "test context");
            assert_eq!(r.code, spec.code);
            assert_eq!(r.exit_code, spec.exit_code);
        }
    }
}
