//! Determinism: BitwisePortable
//! Seeded bias registry, validation, and injection operators (`BD-130`, plan §20.10, §33.3).
//!
//! Covers all six mandatory BD-130 bias families (`resource_model`, `dilution`, `survey`,
//! `sampler`, `density`, `moisture`) plus `truck_payload` (§20.10), with registry-backed bounds,
//! exact wet/dry propagation, and identifiability-confounding checks (`FMINE-REF-104`).

use std::collections::{BTreeMap, BTreeSet};

use fmine_units::{Dry, Mass, MoistureFraction, Neutral, Qty, Wet, wet_to_dry};

use crate::GenError;

/// Canonical seeded bias families required across synthetic worlds (`BD-130`, §20.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BiasFamily {
    ResourceModel,
    Dilution,
    Survey,
    Sampler,
    Density,
    Moisture,
    TruckPayload,
}

impl BiasFamily {
    /// All six mandatory families required by `BD-130` acceptance.
    pub const REQUIRED_BD_130: [Self; 6] = [
        Self::ResourceModel,
        Self::Dilution,
        Self::Survey,
        Self::Sampler,
        Self::Density,
        Self::Moisture,
    ];

    /// Stable registry string key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceModel => "resource_model",
            Self::Dilution => "dilution",
            Self::Survey => "survey",
            Self::Sampler => "sampler",
            Self::Density => "density",
            Self::Moisture => "moisture",
            Self::TruckPayload => "truck_payload",
        }
    }

    fn parse(s: &str) -> Result<Self, GenError> {
        match s {
            "resource_model" => Ok(Self::ResourceModel),
            "dilution" => Ok(Self::Dilution),
            "survey" => Ok(Self::Survey),
            "sampler" => Ok(Self::Sampler),
            "density" => Ok(Self::Density),
            "moisture" => Ok(Self::Moisture),
            "truck_payload" => Ok(Self::TruckPayload),
            other => Err(GenError::InvalidConfig {
                field: "bias.family",
                detail: format!("unknown bias family {other:?}"),
            }),
        }
    }
}

/// One registered bias family entry parsed from `registries/biases.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiasRegistryEntry {
    pub id: String,
    pub family: BiasFamily,
    pub stage: String,
    pub quantity_kind: String,
    pub min_factor_bps: u32,
    pub max_factor_bps: u32,
    pub confounds_with: BiasFamily,
    pub separating_measurement: String,
    pub description: String,
}

/// Complete validated bias registry loaded from `registries/biases.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiasRegistry {
    entries: BTreeMap<BiasFamily, BiasRegistryEntry>,
}

impl BiasRegistry {
    /// Return the entry for a given [`BiasFamily`], if registered.
    #[must_use]
    pub fn get(&self, family: BiasFamily) -> Option<&BiasRegistryEntry> {
        self.entries.get(&family)
    }

    /// Number of registered bias families.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if registry has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate registered entries in canonical family order.
    pub fn entries(&self) -> impl Iterator<Item = &BiasRegistryEntry> {
        self.entries.values()
    }

    /// Check whether `(a, b)` is a confounded pair without its separating measurement (`FMINE-REF-104`).
    pub fn check_pair_identifiable(
        &self,
        a: BiasFamily,
        b: BiasFamily,
        has_separating_measurement: bool,
    ) -> Result<(), GenError> {
        if has_separating_measurement || a == b {
            return Ok(());
        }
        let a_confounds = self.entries.get(&a).is_some_and(|e| e.confounds_with == b);
        let b_confounds = self.entries.get(&b).is_some_and(|e| e.confounds_with == a);
        if a_confounds || b_confounds {
            let sep = self
                .entries
                .get(&a)
                .map_or("independent audit", |e| e.separating_measurement.as_str());
            return Err(GenError::NonIdentifiableProbe {
                detail: format!(
                    "bias pair ({}, {}) is confounded without separating measurement '{sep}'",
                    a.as_str(),
                    b.as_str()
                ),
            });
        }
        Ok(())
    }
}

/// Parse and validate `registries/biases.toml`, refusing missing mandatory BD-130 families.
pub fn parse_bias_registry(text: &str) -> Result<BiasRegistry, GenError> {
    let mut saw_version = false;
    let mut entries = BTreeMap::new();
    let mut seen_ids = BTreeSet::new();
    let mut cur: Option<Vec<(String, String)>> = None;

    let flush = |fields: Vec<(String, String)>,
                 seen_ids: &mut BTreeSet<String>,
                 entries: &mut BTreeMap<BiasFamily, BiasRegistryEntry>|
     -> Result<(), GenError> {
        let get = |k: &'static str| -> Result<String, GenError> {
            fields
                .iter()
                .find(|(fk, _)| fk == k)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| GenError::InvalidConfig {
                    field: k,
                    detail: format!("[[bias]] missing required field '{k}'"),
                })
        };
        let id = get("id")?;
        if id.is_empty() || !seen_ids.insert(id.clone()) {
            return Err(GenError::InvalidConfig {
                field: "bias.id",
                detail: format!("empty or duplicate bias id {id:?}"),
            });
        }
        let family = BiasFamily::parse(&get("family")?)?;
        let confounds_with = BiasFamily::parse(&get("confounds_with")?)?;
        let min_factor_bps: u32 =
            get("min_factor_bps")?
                .parse()
                .map_err(|_| GenError::InvalidConfig {
                    field: "min_factor_bps",
                    detail: format!("invalid u32 for bias '{id}'"),
                })?;
        let max_factor_bps: u32 =
            get("max_factor_bps")?
                .parse()
                .map_err(|_| GenError::InvalidConfig {
                    field: "max_factor_bps",
                    detail: format!("invalid u32 for bias '{id}'"),
                })?;
        if min_factor_bps == 0 || min_factor_bps > 10_000 || max_factor_bps < 10_000 {
            return Err(GenError::InvalidConfig {
                field: "factor_bounds",
                detail: format!(
                    "bias '{id}' bounds [{min_factor_bps}, {max_factor_bps}] must bracket 10000 bps"
                ),
            });
        }
        let entry = BiasRegistryEntry {
            id,
            family,
            stage: get("stage")?,
            quantity_kind: get("quantity_kind")?,
            min_factor_bps,
            max_factor_bps,
            confounds_with,
            separating_measurement: get("separating_measurement")?,
            description: get("description")?,
        };
        if entries.insert(family, entry).is_some() {
            return Err(GenError::InvalidConfig {
                field: "bias.family",
                detail: format!("duplicate bias family '{}'", family.as_str()),
            });
        }
        Ok(())
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s == "[[bias]]" {
            if !saw_version {
                return Err(GenError::InvalidConfig {
                    field: "schema_version",
                    detail: format!("line {line}: schema_version = 1 must precede [[bias]]"),
                });
            }
            if let Some(prev) = cur.take() {
                flush(prev, &mut seen_ids, &mut entries)?;
            }
            cur = Some(Vec::new());
            continue;
        }
        let Some((k, v)) = s.split_once('=') else {
            return Err(GenError::InvalidConfig {
                field: "biases.toml",
                detail: format!("line {line}: expected key = value"),
            });
        };
        let key = k.trim();
        let val = v.trim();
        match cur.as_mut() {
            None => {
                if key == "schema_version" && val == "1" {
                    saw_version = true;
                } else {
                    return Err(GenError::InvalidConfig {
                        field: "schema_version",
                        detail: format!("line {line}: expected schema_version = 1"),
                    });
                }
            }
            Some(fields) => {
                let parsed = val
                    .strip_prefix('"')
                    .and_then(|x| x.strip_suffix('"'))
                    .map_or_else(|| val.to_string(), ToString::to_string);
                fields.push((key.to_string(), parsed));
            }
        }
    }
    if let Some(last) = cur.take() {
        flush(last, &mut seen_ids, &mut entries)?;
    }
    if !saw_version {
        return Err(GenError::InvalidConfig {
            field: "schema_version",
            detail: "missing schema_version = 1".to_string(),
        });
    }
    for req in BiasFamily::REQUIRED_BD_130 {
        if !entries.contains_key(&req) {
            return Err(GenError::InvalidConfig {
                field: "bias_registry",
                detail: format!("missing mandatory BD-130 bias family '{}'", req.as_str()),
            });
        }
    }
    Ok(BiasRegistry { entries })
}

/// Complete multiplicative bias profile across all seven seeded families (`1.0` = unbiased).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeededBiasProfile {
    pub resource_model: f64,
    pub dilution: f64,
    pub survey: f64,
    pub sampler: f64,
    pub density: f64,
    pub moisture: f64,
    pub truck_payload: f64,
}

impl SeededBiasProfile {
    /// Strictly unbiased control profile (all factors = `1.0`).
    #[must_use]
    pub const fn unbiased() -> Self {
        Self {
            resource_model: 1.0,
            dilution: 1.0,
            survey: 1.0,
            sampler: 1.0,
            density: 1.0,
            moisture: 1.0,
            truck_payload: 1.0,
        }
    }

    /// Return the multiplicative factor for one [`BiasFamily`].
    #[must_use]
    pub const fn factor(self, family: BiasFamily) -> f64 {
        match family {
            BiasFamily::ResourceModel => self.resource_model,
            BiasFamily::Dilution => self.dilution,
            BiasFamily::Survey => self.survey,
            BiasFamily::Sampler => self.sampler,
            BiasFamily::Density => self.density,
            BiasFamily::Moisture => self.moisture,
            BiasFamily::TruckPayload => self.truck_payload,
        }
    }

    /// Validate every factor against the registered bounds in `registry`.
    pub fn validate_against(&self, registry: &BiasRegistry) -> Result<(), GenError> {
        for entry in registry.entries() {
            let f = self.factor(entry.family);
            let lo = f64::from(entry.min_factor_bps) / 10_000.0;
            let hi = f64::from(entry.max_factor_bps) / 10_000.0;
            if !f.is_finite() || !(lo..=hi).contains(&f) {
                return Err(GenError::InvalidConfig {
                    field: "seeded_bias_factor",
                    detail: format!(
                        "{} factor {f} outside registered bounds [{lo}, {hi}]",
                        entry.family.as_str()
                    ),
                });
            }
        }
        Ok(())
    }
}

/// True physical lot properties before bias injection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrueOreLot {
    pub volume_m3: f64,
    pub true_density_t_m3: f64,
    pub true_moisture_fraction: f64,
    pub true_grade_g_per_t: f64,
}

/// Observed lot quantities after deterministic application of a [`SeededBiasProfile`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiasedLotObservation {
    pub true_dry_tonnes: f64,
    pub true_wet_tonnes: f64,
    pub resource_model_grade_g_per_t: f64,
    pub diluted_dry_tonnes: f64,
    pub surveyed_stockpile_tonnes: f64,
    pub sampler_grade_g_per_t: f64,
    pub observed_wet_tonnes: f64,
    pub observed_moisture_fraction: f64,
    pub observed_dry_tonnes: f64,
}

/// Deterministically apply all six BD-130 biases (+ `truck_payload`) to a true ore lot.
pub fn apply_seeded_biases(
    lot: TrueOreLot,
    profile: SeededBiasProfile,
    registry: &BiasRegistry,
) -> Result<BiasedLotObservation, GenError> {
    profile.validate_against(registry)?;
    if !(lot.volume_m3.is_finite()
        && lot.volume_m3 > 0.0
        && lot.true_density_t_m3.is_finite()
        && lot.true_density_t_m3 > 0.0
        && lot.true_grade_g_per_t.is_finite()
        && lot.true_grade_g_per_t >= 0.0)
    {
        return Err(GenError::InvalidConfig {
            field: "true_ore_lot",
            detail: "lot dimensions, density, and grade must be finite and physical".to_string(),
        });
    }
    let true_m =
        MoistureFraction::new(lot.true_moisture_fraction).map_err(|e| GenError::InvalidConfig {
            field: "true_moisture_fraction",
            detail: e.to_string(),
        })?;
    let obs_m =
        MoistureFraction::new(lot.true_moisture_fraction * profile.moisture).map_err(|e| {
            GenError::InvalidConfig {
                field: "observed_moisture_fraction",
                detail: e.to_string(),
            }
        })?;

    let true_dry_tonnes = lot.volume_m3 * lot.true_density_t_m3;
    let true_wet_tonnes = true_dry_tonnes / (1.0 - true_m.value());
    let obs_wet_q: Qty<Mass, Wet, Neutral> = Qty::new(true_wet_tonnes * profile.truck_payload);
    let obs_dry_q: Qty<Mass, Dry, Neutral> = wet_to_dry(obs_wet_q, obs_m);

    Ok(BiasedLotObservation {
        true_dry_tonnes,
        true_wet_tonnes,
        resource_model_grade_g_per_t: lot.true_grade_g_per_t * profile.resource_model,
        diluted_dry_tonnes: true_dry_tonnes * profile.dilution,
        surveyed_stockpile_tonnes: lot.volume_m3
            * profile.survey
            * (lot.true_density_t_m3 * profile.density),
        sampler_grade_g_per_t: lot.true_grade_g_per_t * profile.sampler,
        observed_wet_tonnes: obs_wet_q.value(),
        observed_moisture_fraction: obs_m.value(),
        observed_dry_tonnes: obs_dry_q.value(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_seeded_bias_registry_complete_injection_and_confounding_checks() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text =
            std::fs::read_to_string(root.join("registries/biases.toml")).expect("read biases.toml");
        let reg = parse_bias_registry(&text).expect("parse bias registry");
        assert_eq!(reg.len(), 7);
        for req in BiasFamily::REQUIRED_BD_130 {
            assert!(
                reg.get(req).is_some(),
                "missing mandatory BD-130 family {}",
                req.as_str()
            );
        }

        // Unbiased control reproduces true physical quantities to machine precision.
        let lot = TrueOreLot {
            volume_m3: 40_000.0,
            true_density_t_m3: 2.65,
            true_moisture_fraction: 0.08,
            true_grade_g_per_t: 2.40,
        };
        let ctrl =
            apply_seeded_biases(lot, SeededBiasProfile::unbiased(), &reg).expect("unbiased ctrl");
        assert!((ctrl.true_dry_tonnes - 106_000.0).abs() < 1e-9);
        assert!((ctrl.observed_dry_tonnes - ctrl.true_dry_tonnes).abs() < 1e-9);
        assert!((ctrl.surveyed_stockpile_tonnes - ctrl.true_dry_tonnes).abs() < 1e-9);

        // Seeded biases propagate cleanly across all families.
        let biased_profile = SeededBiasProfile {
            resource_model: 1.12,
            dilution: 1.06,
            survey: 0.95,
            sampler: 0.94,
            density: 1.03,
            moisture: 1.25, // observed moisture = 0.10 instead of 0.08
            truck_payload: 1.04,
        };
        let obs = apply_seeded_biases(lot, biased_profile, &reg).expect("biased obs");
        assert!((obs.resource_model_grade_g_per_t - 2.40 * 1.12).abs() < 1e-12);
        assert!((obs.sampler_grade_g_per_t - 2.40 * 0.94).abs() < 1e-12);
        assert!((obs.diluted_dry_tonnes - 106_000.0 * 1.06).abs() < 1e-9);
        assert!((obs.surveyed_stockpile_tonnes - 106_000.0 * 0.95 * 1.03).abs() < 1e-9);
        assert!((obs.observed_moisture_fraction - 0.10).abs() < 1e-12);

        // Out-of-bounds bias refuses with FMINE-REF-109.
        let mut bad = biased_profile;
        bad.sampler = 1.50;
        let err = apply_seeded_biases(lot, bad, &reg).unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-109");

        // Confounded bias pairs refuse attribution with FMINE-REF-104 without separating audit.
        let conf_err = reg
            .check_pair_identifiable(BiasFamily::Survey, BiasFamily::Density, false)
            .unwrap_err();
        assert_eq!(conf_err.to_refusal().code, "FMINE-REF-104");
        reg.check_pair_identifiable(BiasFamily::Survey, BiasFamily::Density, true)
            .expect("separating measurement unblocks identifiability");
        reg.check_pair_identifiable(BiasFamily::Sampler, BiasFamily::Moisture, false)
            .expect("non-confounded pair is identifiable");
    }
}
