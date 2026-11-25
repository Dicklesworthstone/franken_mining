//! Reference-class registry (`REFCLASS-`) and Huber ε-contamination guard (`BD-050`, plan §9.3, `GATE-PLUMB-E4`).
//! Determinism: BitwisePortable
//!
//! Enforces the two `GATE-PLUMB-E4` invariants by type and guard:
//! 1. **Zero ε without a `RefClass`**: [`EpsilonContamination::try_new`] refuses any `epsilon_bps > 0`
//!    that does not cite a registered [`RefClassRecord`] (`FMINE-REF-205`).
//! 2. **Zero unverified reference classes on disclosure-facing claims**: [`RefClassStatus::Unverified`]
//!    records may inform internal analysis, but `ClaimDag::guard_disclosure_export` refuses them and
//!    any descendant derived from them (`FMINE-REF-304`).

use std::collections::BTreeMap;
use std::fmt;

use fmine_core::det::{hypot, sqrt};
use fmine_evidence::{Refusal, RefusalCode};

/// Canonical `REFCLASS-...` identifier (`BD-050`, plan §9.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefClassId(String);

impl RefClassId {
    /// Validate and construct a `REFCLASS-...` identifier.
    ///
    /// Must start with `"REFCLASS-"`, have at least one character after the prefix, be at most 64
    /// bytes, and contain only ASCII uppercase letters, digits, or `'-'`.
    pub fn parse(raw: &str) -> Result<Self, Refusal> {
        let trimmed = raw.trim();
        let valid_chars = trimmed
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-');
        if trimmed != raw
            || !trimmed.starts_with("REFCLASS-")
            || trimmed.len() <= "REFCLASS-".len()
            || trimmed.len() > 64
            || !valid_chars
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("invalid RefClassId {raw:?}: must match REFCLASS-[A-Z0-9-]+ (<= 64 bytes)"),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Underlying `REFCLASS-...` identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Verification status of a `REFCLASS-` record (`§9.3`, `GATE-PLUMB-E4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefClassStatus {
    /// Vetted reference class admissible on both internal and disclosure-facing claims.
    Verified,
    /// Preliminary or unvetted reference class: internal decisions only, blocked by `G-DISC`.
    Unverified,
}

impl RefClassStatus {
    /// Parse `"verified"` or `"unverified"`.
    pub fn parse(raw: &str) -> Result<Self, Refusal> {
        match raw {
            "verified" => Ok(Self::Verified),
            "unverified" => Ok(Self::Unverified),
            other => Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("invalid RefClassStatus {other:?}: expected 'verified' or 'unverified'"),
            )),
        }
    }

    /// Canonical registry spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
        }
    }
}

/// One registered `REFCLASS-` row from `registries/refclass.toml` (`§9.3`, `BD-050`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefClassRecord {
    pub id: RefClassId,
    pub domain: String,
    pub population: String,
    pub n: u32,
    pub period: String,
    pub inclusion_rule: String,
    pub similarity_justification: String,
    pub status: RefClassStatus,
    /// Default contamination weight `ε` in basis points (`0..=10_000`; `2_500` = `0.25`).
    pub epsilon_bps: u16,
    /// Outside-view mean multiplier in basis points (`12_800` = `1.28×`).
    pub ref_mean_multiplier_bps: u32,
    /// Outside-view standard deviation multiplier in basis points (`3_500` = `0.35×`).
    pub ref_std_multiplier_bps: u32,
    pub source_document: String,
    pub source_e_ids: Vec<String>,
}

/// Parsed, canonical `registries/refclass.toml` table keyed by [`RefClassId`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefClassRegistry {
    records: BTreeMap<RefClassId, RefClassRecord>,
}

impl RefClassRegistry {
    /// Parse `registries/refclass.toml` (schema_version = 1), refusing duplicate or malformed rows.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        let mut saw_version = false;
        let mut records = BTreeMap::new();
        let mut cur: Option<BTreeMap<String, String>> = None;

        let flush = |fields: BTreeMap<String, String>,
                     records: &mut BTreeMap<RefClassId, RefClassRecord>|
         -> Result<(), Refusal> {
            let req = |k: &str| -> Result<String, Refusal> {
                fields
                    .get(k)
                    .filter(|v| !v.trim().is_empty())
                    .cloned()
                    .ok_or_else(|| {
                        Refusal::new(
                            RefusalCode::UndeclaredUnitBasisOrTz107,
                            format!(
                                "registries/refclass.toml: [[refclass]] missing non-empty '{k}'"
                            ),
                        )
                    })
            };
            let id = RefClassId::parse(&req("id")?)?;
            if records.contains_key(&id) {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("registries/refclass.toml: duplicate refclass id '{id}'"),
                ));
            }
            let n: u32 = req("n")?.parse().map_err(|_| {
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("refclass '{id}': n must be a positive integer"),
                )
            })?;
            if n == 0 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("refclass '{id}': n must be >= 1"),
                ));
            }
            let status = RefClassStatus::parse(&req("status")?)?;
            let epsilon_bps: u16 = req("epsilon_bps")?.parse().map_err(|_| {
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("refclass '{id}': epsilon_bps must be u16"),
                )
            })?;
            if epsilon_bps > 10_000 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("refclass '{id}': epsilon_bps {epsilon_bps} exceeds 10000"),
                ));
            }
            let ref_mean_multiplier_bps: u32 =
                req("ref_mean_multiplier_bps")?.parse().map_err(|_| {
                    Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("refclass '{id}': invalid ref_mean_multiplier_bps"),
                    )
                })?;
            let ref_std_multiplier_bps: u32 =
                req("ref_std_multiplier_bps")?.parse().map_err(|_| {
                    Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("refclass '{id}': invalid ref_std_multiplier_bps"),
                    )
                })?;
            if ref_mean_multiplier_bps == 0 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("refclass '{id}': ref_mean_multiplier_bps must be > 0"),
                ));
            }
            let source_e_ids: Vec<String> = req("source_e_ids")?
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect();
            if source_e_ids.is_empty() {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("refclass '{id}': must cite at least one source_e_id"),
                ));
            }
            records.insert(
                id.clone(),
                RefClassRecord {
                    id,
                    domain: req("domain")?,
                    population: req("population")?,
                    n,
                    period: req("period")?,
                    inclusion_rule: req("inclusion_rule")?,
                    similarity_justification: req("similarity_justification")?,
                    status,
                    epsilon_bps,
                    ref_mean_multiplier_bps,
                    ref_std_multiplier_bps,
                    source_document: req("source_document")?,
                    source_e_ids,
                },
            );
            Ok(())
        };

        for (idx, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[[refclass]]" {
                if !saw_version {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!(
                            "registries/refclass.toml:{}: schema_version = 1 must precede [[refclass]]",
                            idx + 1
                        ),
                    ));
                }
                if let Some(prev) = cur.take() {
                    flush(prev, &mut records)?;
                }
                cur = Some(BTreeMap::new());
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("registries/refclass.toml:{}: expected key = value", idx + 1),
                ));
            };
            let key = k.trim();
            let val = v.trim();
            match cur.as_mut() {
                None => {
                    if !saw_version && key == "schema_version" && val == "1" {
                        saw_version = true;
                    } else {
                        return Err(Refusal::new(
                            RefusalCode::UndeclaredUnitBasisOrTz107,
                            format!(
                                "registries/refclass.toml:{}: unexpected top-level line {line:?}",
                                idx + 1
                            ),
                        ));
                    }
                }
                Some(map) => {
                    let parsed = if let Some(inner) =
                        val.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
                    {
                        inner.to_string()
                    } else {
                        val.to_string()
                    };
                    if map.insert(key.to_string(), parsed).is_some() {
                        return Err(Refusal::new(
                            RefusalCode::UndeclaredUnitBasisOrTz107,
                            format!(
                                "registries/refclass.toml:{}: duplicate field '{key}'",
                                idx + 1
                            ),
                        ));
                    }
                }
            }
        }
        if let Some(last) = cur.take() {
            flush(last, &mut records)?;
        }
        if !saw_version || records.is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "registries/refclass.toml requires schema_version = 1 and at least one [[refclass]] row",
            ));
        }
        Ok(Self { records })
    }

    /// Look up a `REFCLASS-` row by exact id.
    #[must_use]
    pub fn get(&self, id: &RefClassId) -> Option<&RefClassRecord> {
        self.records.get(id)
    }

    /// Look up by string id, returning `FMINE-REF-201` if absent or malformed.
    pub fn lookup(&self, raw_id: &str) -> Result<&RefClassRecord, Refusal> {
        let id = RefClassId::parse(raw_id)?;
        self.records.get(&id).ok_or_else(|| {
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("reference class '{id}' is not registered in registries/refclass.toml"),
            )
        })
    }

    /// Number of registered reference-class rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Iterate over all rows in deterministic ascending `RefClassId` order.
    pub fn iter(&self) -> impl Iterator<Item = &RefClassRecord> {
        self.records.values()
    }
}

/// Checked Huber ε-contamination attachment (`GATE-PLUMB-E4`, plan §9.3).
///
/// Fields are private so no caller can construct `epsilon_bps > 0` without a registered
/// [`RefClassRecord`]:
///
/// ```compile_fail,E0451
/// let _forged = fmine_claim::refclass::EpsilonContamination {
///     ref_class: None,
///     epsilon_bps: 2500,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpsilonContamination {
    ref_class: Option<RefClassRecord>,
    epsilon_bps: u16,
}

impl Default for EpsilonContamination {
    fn default() -> Self {
        Self::none()
    }
}

impl EpsilonContamination {
    /// Zero contamination (`ε = 0`, no reference class attached).
    #[must_use]
    pub const fn none() -> Self {
        Self {
            ref_class: None,
            epsilon_bps: 0,
        }
    }

    /// Construct a checked contamination record enforcing `GATE-PLUMB-E4`:
    /// - refuses `epsilon_bps > 10_000` (`FMINE-REF-109`);
    /// - refuses `epsilon_bps > 0` with `ref_class == None` (`FMINE-REF-205`).
    pub fn try_new(ref_class: Option<RefClassRecord>, epsilon_bps: u16) -> Result<Self, Refusal> {
        if epsilon_bps > 10_000 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("GATE-PLUMB-E4: epsilon_bps ({epsilon_bps}) exceeds 10000"),
            ));
        }
        if epsilon_bps > 0 && ref_class.is_none() {
            return Err(Refusal::new(
                RefusalCode::CalibrationRequired205,
                "GATE-PLUMB-E4: non-zero epsilon contamination requires a registered REFCLASS- record",
            ));
        }
        Ok(Self {
            ref_class,
            epsilon_bps,
        })
    }

    /// Build from a [`RefClassRegistry`] entry, using the row's default `epsilon_bps` unless
    /// `override_epsilon_bps` is explicitly supplied (`0..=10_000`).
    pub fn from_registry(
        registry: &RefClassRegistry,
        raw_id: &str,
        override_epsilon_bps: Option<u16>,
    ) -> Result<Self, Refusal> {
        let record = registry.lookup(raw_id)?.clone();
        let eps = override_epsilon_bps.unwrap_or(record.epsilon_bps);
        Self::try_new(Some(record), eps)
    }

    /// Attached reference-class record, if any.
    #[must_use]
    pub fn ref_class(&self) -> Option<&RefClassRecord> {
        self.ref_class.as_ref()
    }

    /// Contamination weight `ε` in basis points (`0..=10_000`).
    #[must_use]
    pub const fn epsilon_bps(&self) -> u16 {
        self.epsilon_bps
    }

    /// Contamination weight `ε` as a finite fraction in `[0.0, 1.0]`.
    #[must_use]
    pub fn epsilon(&self) -> f64 {
        f64::from(self.epsilon_bps) / 10_000.0
    }

    /// Status of the attached reference class, if any.
    #[must_use]
    pub fn status(&self) -> Option<RefClassStatus> {
        self.ref_class.as_ref().map(|r| r.status)
    }

    /// Moment-matched Huber ε-contaminated mixture `(1 - ε) * P_model + ε * Q_ref` (`GB-32`, §9.3).
    ///
    /// Uses `fmine_core::det::{sqrt, hypot}` (`BitwisePortable`). Refuses non-finite inputs or
    /// negative `base_std` with `FMINE-REF-109`. Returns `(base_mean, base_std)` unchanged when
    /// `epsilon_bps == 0`.
    pub fn blend_moments(&self, base_mean: f64, base_std: f64) -> Result<(f64, f64), Refusal> {
        if !base_mean.is_finite() || !base_std.is_finite() || base_std < 0.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "reference-class blend requires finite base_mean and finite non-negative base_std",
            ));
        }
        if self.epsilon_bps == 0 {
            return Ok((base_mean, base_std));
        }
        let Some(rec) = &self.ref_class else {
            return Err(Refusal::new(
                RefusalCode::CalibrationRequired205,
                "GATE-PLUMB-E4: cannot blend non-zero epsilon without a RefClassRecord",
            ));
        };
        let eps = self.epsilon();
        let mean_mult = f64::from(rec.ref_mean_multiplier_bps) / 10_000.0;
        let std_mult = f64::from(rec.ref_std_multiplier_bps) / 10_000.0;
        let ref_mean = base_mean * mean_mult;
        let ref_std = base_mean.abs() * std_mult;
        if !ref_mean.is_finite() || !ref_std.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "reference-class moments overflowed f64",
            ));
        }
        if self.epsilon_bps == 10_000 {
            return Ok((ref_mean, ref_std));
        }
        let blended_mean = (1.0 - eps) * base_mean + eps * ref_mean;
        let within = hypot(sqrt(1.0 - eps) * base_std, sqrt(eps) * ref_std);
        let between = sqrt(eps * (1.0 - eps)) * (ref_mean - base_mean).abs();
        let blended_std = hypot(within, between);
        if !blended_mean.is_finite() || !blended_std.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "blended reference-class moments overflowed f64",
            ));
        }
        Ok((blended_mean, blended_std))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load_workspace_registry() -> RefClassRegistry {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text =
            std::fs::read_to_string(root.join("registries/refclass.toml")).expect("read refclass");
        RefClassRegistry::parse(&text).expect("parse workspace registries/refclass.toml")
    }

    #[test]
    fn test_gate_plumb_e4_zero_eps_without_refclass_and_gb32_blend() {
        let reg = load_workspace_registry();
        assert!(reg.len() >= 6);

        // GATE-PLUMB-E4 clause 1: non-zero epsilon without RefClass MUST refuse (FMINE-REF-205).
        let err = EpsilonContamination::try_new(None, 2500)
            .expect_err("non-zero epsilon without RefClass must refuse");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-205", 3));

        // Zero epsilon without RefClass is admissible and acts as identity.
        let zero = EpsilonContamination::try_new(None, 0).expect("zero epsilon without refclass");
        assert_eq!(zero.blend_moments(1000.0, 120.0).unwrap(), (1000.0, 120.0));

        // Out-of-range epsilon > 10000 refuses (FMINE-REF-109, no silent clamping).
        assert_eq!(
            EpsilonContamination::try_new(None, 10_001)
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );

        // Unregistered RefClass refuses with FMINE-REF-201.
        assert_eq!(
            EpsilonContamination::from_registry(&reg, "REFCLASS-UNKNOWN-999", None)
                .unwrap_err()
                .code,
            "FMINE-REF-201"
        );

        // GB-32 known answer: REFCLASS-CAPEX-GREENFIELD (eps=0.25, mean_mult=1.28, std_mult=0.35)
        // on base N(1000, 100^2):
        //   blended_mean = 0.75 * 1000 + 0.25 * 1280 = 1070.0
        //   blended_var  = 0.75*(100^2 + 1000^2) + 0.25*(350^2 + 1280^2) - 1070^2
        //                = 0.75*10000 + 0.25*122500 + 0.75*0.25*(280^2)
        //                = 7500 + 30625 + 14700 = 52825 => sd = sqrt(52825) = 229.83689869...
        let greenfield =
            EpsilonContamination::from_registry(&reg, "REFCLASS-CAPEX-GREENFIELD", None).unwrap();
        assert_eq!(greenfield.status(), Some(RefClassStatus::Verified));
        let (mu, sd) = greenfield.blend_moments(1000.0, 100.0).unwrap();
        assert!((mu - 1070.0).abs() < 1e-12, "mu={mu}");
        assert!((sd - 52_825.0_f64.sqrt()).abs() < 1e-11, "sd={sd}");

        // Unverified row parses cleanly and carries RefClassStatus::Unverified.
        let unverified =
            EpsilonContamination::from_registry(&reg, "REFCLASS-TAILINGS-PRELIMINARY", None)
                .unwrap();
        assert_eq!(unverified.status(), Some(RefClassStatus::Unverified));
    }

    #[test]
    fn test_refclass_registry_refuses_malformed_and_duplicate_rows() {
        assert!(RefClassId::parse("CAPEX-GREENFIELD").is_err());
        assert!(RefClassId::parse("REFCLASS-").is_err());
        assert!(RefClassId::parse("REFCLASS-lower").is_err());
        assert!(RefClassStatus::parse("draft").is_err());

        let dup = r#"
schema_version = 1
[[refclass]]
id = "REFCLASS-DUP"
domain = "d"
population = "p"
n = 10
period = "2000-2020"
inclusion_rule = "r"
similarity_justification = "j"
status = "verified"
epsilon_bps = 1000
ref_mean_multiplier_bps = 11000
ref_std_multiplier_bps = 2000
source_document = "doc"
source_e_ids = "E001"

[[refclass]]
id = "REFCLASS-DUP"
domain = "d"
population = "p"
n = 10
period = "2000-2020"
inclusion_rule = "r"
similarity_justification = "j"
status = "verified"
epsilon_bps = 1000
ref_mean_multiplier_bps = 11000
ref_std_multiplier_bps = 2000
source_document = "doc"
source_e_ids = "E001"
"#;
        assert_eq!(
            RefClassRegistry::parse(dup).unwrap_err().code,
            "FMINE-REF-107"
        );
    }
}
