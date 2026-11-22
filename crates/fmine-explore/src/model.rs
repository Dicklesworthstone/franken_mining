//! Determinism: BitwisePortable
//! Lodestar exploration data model: deposits, effort records, mineral-system ingredients,
//! IDEA elicitation records, and versioned tract maps (`BD-231`, plan §11.3).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::anchor::SourceAnchor;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_crs::CrsId;
use fmine_units::ExactQty;

use crate::ExploreError;

/// Exploration survey or drilling method from plan §11.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExploreMethod {
    AirborneMagnetics,
    Gravity,
    Aem,
    IpResistivity,
    Magnetotellurics,
    Multispectral,
    StreamSediment,
    SoilGeochem,
    GeologicalMapping,
    RabDrilling,
    AircoreDrilling,
    RcDrilling,
    DiamondDrilling,
}

/// Mineral-systems framework ingredient (plan §11.3–§11.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ingredient {
    Source,
    Energy,
    Pathway,
    Trap,
    Preservation,
    Cover,
}

/// Completeness and geometric reliability of a historical exploration effort record (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoverageConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

/// CRS-tagged exact coordinate and local transported-cover depth (zero bare `f64`; L-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeoCoord {
    pub crs: CrsId,
    pub easting_m: ExactQty,
    pub northing_m: ExactQty,
    pub elevation_m: ExactQty,
    /// Thickness of transported post-mineral cover at this location (`>= 0` m).
    pub cover_depth_m: ExactQty,
}

impl GeoCoord {
    /// Construct a validated CRS-tagged coordinate requiring `cover_depth_m >= 0`.
    pub fn new(
        crs: CrsId,
        easting_m: ExactQty,
        northing_m: ExactQty,
        elevation_m: ExactQty,
        cover_depth_m: ExactQty,
    ) -> Result<Self, ExploreError> {
        if cover_depth_m.signum() < 0 {
            return Err(ExploreError::InvalidRecord {
                field: "cover_depth_m",
                detail: "cover_depth_m must be >= 0".to_string(),
            });
        }
        Ok(Self {
            crs,
            easting_m,
            northing_m,
            elevation_m,
            cover_depth_m,
        })
    }

    /// Exact squared planar horizontal distance between two coordinates in the same `CrsId`.
    /// Refuses cross-CRS comparison with `FMINE-REF-106` (`G-CRS-1`).
    pub fn planar_dist_sq(&self, other: &Self) -> Result<ExactQty, ExploreError> {
        if self.crs != other.crs {
            return Err(ExploreError::CrsMismatch {
                left: self.crs,
                right: other.crs,
            });
        }
        let dx = self.easting_m.checked_sub(other.easting_m)?;
        let dy = self.northing_m.checked_sub(other.northing_m)?;
        Ok(dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)?)
    }
}

/// Deposit-type probability vector over named catalogue types PLUS mandatory `k*` novel/unclassified
/// Good–Turing UU slot (`§11.3–§11.4`, `GB-09`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositTypeProbs {
    pub named: BTreeMap<String, ExactQty>,
    /// Probability mass reserved for novel/unclassified deposit type `k*` (UU).
    pub k_star_novel: ExactQty,
}

impl DepositTypeProbs {
    /// Build a validated simplex summing exactly to `1`, with non-negative weights and `>= 1` named type.
    pub fn new(
        named: BTreeMap<String, ExactQty>,
        k_star_novel: ExactQty,
    ) -> Result<Self, ExploreError> {
        if named.is_empty() {
            return Err(ExploreError::InvalidSimplex {
                detail: "DepositTypeProbs requires at least one named deposit type".to_string(),
            });
        }
        if k_star_novel.signum() < 0 {
            return Err(ExploreError::InvalidSimplex {
                detail: "k_star_novel probability must be >= 0".to_string(),
            });
        }
        let mut sum = k_star_novel;
        for (name, &p) in &named {
            if name.trim().is_empty() {
                return Err(ExploreError::InvalidSimplex {
                    detail: "named deposit type key must be non-empty".to_string(),
                });
            }
            if p.signum() < 0 {
                return Err(ExploreError::InvalidSimplex {
                    detail: format!("negative probability for deposit type '{name}'"),
                });
            }
            sum = sum.checked_add(p)?;
        }
        if sum != ExactQty::from_integer(1) {
            return Err(ExploreError::InvalidSimplex {
                detail: format!(
                    "deposit type simplex including k* must sum to 1, got {}/{}",
                    sum.num(),
                    sum.den()
                ),
            });
        }
        Ok(Self {
            named,
            k_star_novel,
        })
    }
}

/// Quantitative marks of a deposit or occurrence (tonnage, grade, depth, style, censoring flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkRecord {
    pub tonnage_mt: ExactQty,
    pub grade_g_per_t: ExactQty,
    pub depth_m: ExactQty,
    pub style: String,
    /// True when the occurrence is sub-economic / right-censored (`A3FoundSubEconomic`, §11.3).
    pub censored_sub_economic: bool,
}

impl MarkRecord {
    /// Construct a validated physical `MarkRecord`.
    pub fn new(
        tonnage_mt: ExactQty,
        grade_g_per_t: ExactQty,
        depth_m: ExactQty,
        style: impl Into<String>,
        censored_sub_economic: bool,
    ) -> Result<Self, ExploreError> {
        let style = style.into();
        if style.trim().is_empty() {
            return Err(ExploreError::InvalidRecord {
                field: "mark.style",
                detail: "style must be non-empty".to_string(),
            });
        }
        if tonnage_mt.signum() < 0 || grade_g_per_t.signum() < 0 || depth_m.signum() < 0 {
            return Err(ExploreError::InvalidRecord {
                field: "mark",
                detail: "tonnage_mt, grade_g_per_t, and depth_m must be >= 0".to_string(),
            });
        }
        Ok(Self {
            tonnage_mt,
            grade_g_per_t,
            depth_m,
            style,
            censored_sub_economic,
        })
    }
}

/// Discovery attribution metadata for a known deposit (`§11.3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryEvent {
    pub year: u16,
    pub method: ExploreMethod,
    pub source: SourceAnchor,
}

/// Deposit or sub-economic mineral occurrence record (`BD-231`, §11.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositRecord {
    pub id: ObjectId,
    pub name: String,
    pub coord: GeoCoord,
    pub deposit_type: DepositTypeProbs,
    pub marks: MarkRecord,
    pub discovery: Option<DiscoveryEvent>,
    /// USGS aggregation rule id ensuring consistent spatial grouping of labels, marks, and outputs.
    pub aggregation_rule_id: String,
    pub provenance: Vec<SourceAnchor>,
}

impl DepositRecord {
    /// Construct a validated [`DepositRecord`].
    #[allow(
        clippy::too_many_arguments,
        reason = "domain-complete DepositRecord constructor per plan §11.3"
    )]
    pub fn build(
        id: ObjectId,
        name: impl Into<String>,
        coord: GeoCoord,
        deposit_type: DepositTypeProbs,
        marks: MarkRecord,
        discovery: Option<DiscoveryEvent>,
        aggregation_rule_id: impl Into<String>,
        provenance: Vec<SourceAnchor>,
    ) -> Result<Self, ExploreError> {
        let name = name.into();
        let aggregation_rule_id = aggregation_rule_id.into();
        if name.trim().is_empty() {
            return Err(ExploreError::InvalidRecord {
                field: "deposit.name",
                detail: "name must be non-empty".to_string(),
            });
        }
        if aggregation_rule_id.trim().is_empty() {
            return Err(ExploreError::MissingProvenance {
                field: "aggregation_rule_id",
                detail: "USGS aggregation_rule_id is required".to_string(),
            });
        }
        if provenance.is_empty() {
            return Err(ExploreError::MissingProvenance {
                field: "provenance",
                detail: "DepositRecord requires >= 1 SourceAnchor".to_string(),
            });
        }
        Ok(Self {
            id,
            name,
            coord,
            deposit_type,
            marks,
            discovery,
            aggregation_rule_id,
            provenance,
        })
    }
}

/// Physical method specification controlling detection probability `q_j` (`§11.3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSpec {
    /// Effective survey radius / footprint half-width in metres (`> 0`).
    pub coverage_radius_m: ExactQty,
    /// Line or collar spacing in metres (`> 0`).
    pub spacing_m: ExactQty,
    /// Maximum depth of investigation (DOI) through cover in metres (`> 0`).
    pub depth_of_investigation_m: ExactQty,
    /// Baseline era/method detection efficiency `q_bar ∈ (0, 1]` at zero cover.
    pub baseline_q: ExactQty,
    /// Effort intensity multiplier `e_j > 0` (e.g. passes, drill-metres per km²).
    pub effort_intensity: ExactQty,
}

impl MethodSpec {
    /// Construct a validated [`MethodSpec`].
    pub fn new(
        coverage_radius_m: ExactQty,
        spacing_m: ExactQty,
        depth_of_investigation_m: ExactQty,
        baseline_q: ExactQty,
        effort_intensity: ExactQty,
    ) -> Result<Self, ExploreError> {
        let one = ExactQty::from_integer(1);
        if coverage_radius_m.signum() <= 0
            || spacing_m.signum() <= 0
            || depth_of_investigation_m.signum() <= 0
            || effort_intensity.signum() <= 0
            || baseline_q.signum() <= 0
            || baseline_q.checked_cmp(one)? == core::cmp::Ordering::Greater
        {
            return Err(ExploreError::InvalidRecord {
                field: "method_spec",
                detail:
                    "radius, spacing, DOI, effort_intensity > 0 and baseline_q in (0, 1] required"
                        .to_string(),
            });
        }
        Ok(Self {
            coverage_radius_m,
            spacing_m,
            depth_of_investigation_m,
            baseline_q,
            effort_intensity,
        })
    }
}

/// Recorded historical exploration effort over a CRS-bound footprint (`BD-231`, §11.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortRecord {
    pub id: ObjectId,
    pub method: ExploreMethod,
    pub center: GeoCoord,
    pub start_year: u16,
    pub end_year: u16,
    pub spec: MethodSpec,
    pub source: SourceAnchor,
    pub coverage: CoverageConfidence,
}

impl EffortRecord {
    /// Construct a validated [`EffortRecord`].
    #[allow(
        clippy::too_many_arguments,
        reason = "domain-complete EffortRecord constructor per plan §11.3"
    )]
    pub fn build(
        id: ObjectId,
        method: ExploreMethod,
        center: GeoCoord,
        start_year: u16,
        end_year: u16,
        spec: MethodSpec,
        source: SourceAnchor,
        coverage: CoverageConfidence,
    ) -> Result<Self, ExploreError> {
        if start_year > end_year {
            return Err(ExploreError::InvalidRecord {
                field: "valid_years",
                detail: format!("start_year ({start_year}) > end_year ({end_year})"),
            });
        }
        if source.artifact_path.trim().is_empty() || source.locator.trim().is_empty() {
            return Err(ExploreError::MissingProvenance {
                field: "effort.source",
                detail: "EffortRecord requires non-empty SourceAnchor".to_string(),
            });
        }
        Ok(Self {
            id,
            method,
            center,
            start_year,
            end_year,
            spec,
            source,
            coverage,
        })
    }
}

/// Structured IDEA-protocol expert elicitation record (`N90 <= N50 <= N10`, §11.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElicitationRecord {
    pub tract_id: String,
    /// Anonymous expert count (never stores personal names; AGENTS.md privacy rule).
    pub expert_count: u32,
    /// `[n90, n50, n10]` undiscovered-deposit count quantiles satisfying `n90 <= n50 <= n10`.
    pub n90_n50_n10: [u32; 3],
    /// Scored seed-question calibration record (`PrefixKind::Calibration`).
    pub seeds_scored: ObjectId,
}

impl ElicitationRecord {
    /// Construct a validated IDEA-protocol elicitation record.
    pub fn build(
        tract_id: impl Into<String>,
        expert_count: u32,
        n90_n50_n10: [u32; 3],
        seeds_scored: ObjectId,
    ) -> Result<Self, ExploreError> {
        let tract_id = tract_id.into();
        if tract_id.trim().is_empty() || expert_count == 0 {
            return Err(ExploreError::InvalidRecord {
                field: "elicitation",
                detail: "tract_id must be non-empty and expert_count >= 1".to_string(),
            });
        }
        if !(n90_n50_n10[0] <= n90_n50_n10[1] && n90_n50_n10[1] <= n90_n50_n10[2]) {
            return Err(ExploreError::InvalidRecord {
                field: "n90_n50_n10",
                detail: format!(
                    "elicited quantiles must satisfy N90 <= N50 <= N10, got {:?}",
                    n90_n50_n10
                ),
            });
        }
        if seeds_scored.kind != PrefixKind::Calibration {
            return Err(ExploreError::InvalidRecord {
                field: "seeds_scored",
                detail: "seeds_scored must have PrefixKind::Calibration".to_string(),
            });
        }
        Ok(Self {
            tract_id,
            expert_count,
            n90_n50_n10,
            seeds_scored,
        })
    }
}

/// One permissive mineral tract inside a versioned [`TractMap`] (`§11.3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tract {
    pub tract_id: String,
    pub deposit_model: String,
    pub area_km2: ExactQty,
    pub crs: CrsId,
}

/// Versioned permissive-tract delineation map (`§11.3`; map uncertainty is KU-structural).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TractMap {
    pub map_id: String,
    pub version: u32,
    pub alternatives_id: String,
    pub tracts: Vec<Tract>,
}

impl TractMap {
    /// Construct a validated [`TractMap`] requiring unique non-empty tract IDs and positive areas.
    pub fn build(
        map_id: impl Into<String>,
        version: u32,
        alternatives_id: impl Into<String>,
        tracts: Vec<Tract>,
    ) -> Result<Self, ExploreError> {
        let map_id = map_id.into();
        let alternatives_id = alternatives_id.into();
        if map_id.trim().is_empty()
            || version == 0
            || alternatives_id.trim().is_empty()
            || tracts.is_empty()
        {
            return Err(ExploreError::InvalidRecord {
                field: "tract_map",
                detail: "map_id, version >= 1, alternatives_id, and >= 1 Tract required"
                    .to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        for t in &tracts {
            if t.tract_id.trim().is_empty()
                || t.deposit_model.trim().is_empty()
                || t.area_km2.signum() <= 0
                || !seen.insert(t.tract_id.as_str())
            {
                return Err(ExploreError::InvalidRecord {
                    field: "tract",
                    detail: format!("invalid or duplicate tract '{}'", t.tract_id),
                });
            }
        }
        Ok(Self {
            map_id,
            version,
            alternatives_id,
            tracts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    #[test]
    fn test_explore_data_model_simplex_crs_and_elicitation() {
        let mut named = BTreeMap::new();
        named.insert("carlin_au".to_string(), ExactQty::new(85, 100).unwrap());
        named.insert(
            "porphyry_cu_au".to_string(),
            ExactQty::new(10, 100).unwrap(),
        );
        let probs = DepositTypeProbs::new(named.clone(), ExactQty::new(5, 100).unwrap())
            .expect("valid simplex with k* = 5%");
        assert_eq!(probs.k_star_novel, ExactQty::new(5, 100).unwrap());

        // Simplex not summing to 1 refuses.
        assert!(DepositTypeProbs::new(named, ExactQty::new(10, 100).unwrap()).is_err());

        // CRS mismatch refuses with FMINE-REF-106 (G-CRS-1).
        let p_ngm = GeoCoord::new(
            CrsId::NGM_UTM11N,
            ExactQty::from_integer(550_000),
            ExactQty::from_integer(4_530_000),
            ExactQty::from_integer(1_600),
            ExactQty::from_integer(25),
        )
        .unwrap();
        let p_rd = GeoCoord::new(
            CrsId::REKO_DIQ_UTM41N,
            ExactQty::from_integer(400_000),
            ExactQty::from_integer(3_220_000),
            ExactQty::from_integer(950),
            ExactQty::from_integer(10),
        )
        .unwrap();
        assert_eq!(
            p_ngm.planar_dist_sq(&p_rd).unwrap_err().to_refusal().code,
            "FMINE-REF-106"
        );

        // IDEA elicitation requires N90 <= N50 <= N10 and PrefixKind::Calibration.
        let cal_id = ObjectId::from_canonical_bytes(PrefixKind::Calibration, b"idea-2026");
        let elic = ElicitationRecord::build("TRACT-CARLIN-N", 7, [1, 3, 8], cal_id).expect("elic");
        assert_eq!(elic.n90_n50_n10, [1, 3, 8]);
        assert!(ElicitationRecord::build("TRACT-CARLIN-N", 7, [4, 2, 8], cal_id).is_err());
        let bad_id = ObjectId::from_canonical_bytes(PrefixKind::Claim, b"wrong");
        assert!(ElicitationRecord::build("TRACT-CARLIN-N", 7, [1, 3, 8], bad_id).is_err());

        // DepositRecord + TractMap build cleanly with SourceAnchor provenance.
        let anchor = SourceAnchor::new("usgs/nevada_2026.csv", sha256(b"usgs").0, "row:42");
        let dep = DepositRecord::build(
            ObjectId::from_canonical_bytes(PrefixKind::Claim, b"dep-fourmile"),
            "Fourmile",
            p_ngm,
            probs,
            MarkRecord::new(
                ExactQty::from_integer(20),
                ExactQty::from_integer(15),
                ExactQty::from_integer(600),
                "Carlin-type refractory",
                false,
            )
            .unwrap(),
            Some(DiscoveryEvent {
                year: 2018,
                method: ExploreMethod::DiamondDrilling,
                source: anchor.clone(),
            }),
            "USGS-2km-aggregate-v1",
            vec![anchor],
        )
        .expect("deposit record");
        assert_eq!(dep.name, "Fourmile");

        let tmap = TractMap::build(
            "NEVADA-CARLIN-2026",
            1,
            "HYP-MAP-SET-1",
            vec![Tract {
                tract_id: "TRACT-CARLIN-N".to_string(),
                deposit_model: "carlin_au".to_string(),
                area_km2: ExactQty::from_integer(420),
                crs: CrsId::NGM_UTM11N,
            }],
        )
        .expect("tract map");
        assert_eq!(tmap.tracts.len(), 1);
    }
}
