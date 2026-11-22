//! Determinism: BitwisePortable
//! Sealed geomaterial corpus in `frankensim.matdb-manifest.v1` format (`BD-241`, plan §13.10).
//!
//! Client test data is confidential and MNPI-adjacent, so only sealed private corpora are
//! admitted (`seal_corpus`; `MaterialsError::CorpusChanged` on any tamper), never mutable
//! unsealed seed bundles. Every entry carries its `SourceAnchor` provenance and `Confidentiality`
//! label; queries outside the corpus refuse with `FMINE-REF-201` (`NoClaimInDomain`).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_claim::Confidentiality;
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::EvidenceClass;
use fmine_units::ExactQty;

use crate::MaterialsError;

/// Canonical manifest schema identifier aligned with `frankensim` `fs-matdb` (§13.10).
pub const MATDB_MANIFEST_SCHEMA: &str = "frankensim.matdb-manifest.v1";

/// Non-spatial material classes and L4 domain summary export class (plan §13.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialClass {
    MillLinerSteel,
    GrindingMedia,
    Reagent,
    ProcessWater,
    CertifiedReferenceMaterial,
    TsfConstructionZone,
    Geomembrane,
    Explosive,
    DomainSummary,
}

impl MaterialClass {
    /// Stable lowercase identifier emitted into `frankensim.matdb-manifest.v1`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MillLinerSteel => "mill_liner_steel",
            Self::GrindingMedia => "grinding_media",
            Self::Reagent => "reagent",
            Self::ProcessWater => "process_water",
            Self::CertifiedReferenceMaterial => "certified_reference_material",
            Self::TsfConstructionZone => "tsf_construction_zone",
            Self::Geomembrane => "geomembrane",
            Self::Explosive => "explosive",
            Self::DomainSummary => "domain_summary",
        }
    }
}

/// One exact-rational parameter interval `[lo, hi]` with nominal value and explicit unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialParam {
    pub property_key: String,
    pub unit: String,
    pub lo: ExactQty,
    pub nominal: ExactQty,
    pub hi: ExactQty,
}

impl MaterialParam {
    /// Build a validated parameter enclosure satisfying `lo <= nominal <= hi`.
    pub fn new(
        property_key: impl Into<String>,
        unit: impl Into<String>,
        lo: ExactQty,
        nominal: ExactQty,
        hi: ExactQty,
    ) -> Result<Self, MaterialsError> {
        let property_key = property_key.into();
        let unit = unit.into();
        if property_key.trim().is_empty() || unit.trim().is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "material_param",
                detail: "property_key and unit must be non-empty".to_string(),
            });
        }
        if lo.checked_cmp(nominal)? == core::cmp::Ordering::Greater
            || nominal.checked_cmp(hi)? == core::cmp::Ordering::Greater
        {
            return Err(MaterialsError::InvalidSpec {
                field: "material_param.bounds",
                detail: format!("property '{property_key}' must satisfy lo <= nominal <= hi"),
            });
        }
        Ok(Self {
            property_key,
            unit,
            lo,
            nominal,
            hi,
        })
    }
}

/// One material card entry inside a sealed geomaterial corpus (`BD-241`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialEntry {
    pub material_id: String,
    pub class: MaterialClass,
    pub evidence: EvidenceClass,
    pub confidentiality: Confidentiality,
    pub provenance: Vec<SourceAnchor>,
    pub citation: String,
    pub params: BTreeMap<String, MaterialParam>,
}

impl MaterialEntry {
    /// Construct a validated material entry requiring at least one `SourceAnchor` and one parameter.
    pub fn build(
        material_id: impl Into<String>,
        class: MaterialClass,
        evidence: EvidenceClass,
        confidentiality: Confidentiality,
        provenance: Vec<SourceAnchor>,
        citation: impl Into<String>,
        params: Vec<MaterialParam>,
    ) -> Result<Self, MaterialsError> {
        let material_id = material_id.into();
        let citation = citation.into();
        if material_id.trim().is_empty() || citation.trim().is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "material_entry",
                detail: "material_id and citation must be non-empty".to_string(),
            });
        }
        if provenance.is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "provenance",
                detail: format!("material '{material_id}' requires >= 1 SourceAnchor"),
            });
        }
        for anchor in &provenance {
            if anchor.artifact_path.trim().is_empty() || anchor.locator.trim().is_empty() {
                return Err(MaterialsError::InvalidSpec {
                    field: "provenance.anchor",
                    detail: format!("material '{material_id}' has empty artifact_path or locator"),
                });
            }
        }
        if params.is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "params",
                detail: format!("material '{material_id}' requires >= 1 MaterialParam"),
            });
        }
        let mut map = BTreeMap::new();
        for p in params {
            // Re-validate bounds in case struct fields were mutated directly.
            let _ = MaterialParam::new(
                p.property_key.clone(),
                p.unit.clone(),
                p.lo,
                p.nominal,
                p.hi,
            )?;
            if map.insert(p.property_key.clone(), p).is_some() {
                return Err(MaterialsError::InvalidSpec {
                    field: "params",
                    detail: format!("duplicate property in material '{material_id}'"),
                });
            }
        }
        Ok(Self {
            material_id,
            class,
            evidence,
            confidentiality,
            provenance,
            citation,
            params: map,
        })
    }
}

/// Content-addressed, tamper-evident sealed geomaterial corpus in `frankensim.matdb-manifest.v1`
/// format (`BD-241`, §13.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCorpus {
    pub corpus_id: String,
    pub pack_version: u32,
    entries: BTreeMap<String, MaterialEntry>,
    manifest_json: String,
    corpus_seal: Digest32,
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn evidence_label(ev: EvidenceClass) -> &'static str {
    match ev {
        EvidenceClass::Measured => "measured",
        EvidenceClass::Derived => "derived",
        EvidenceClass::ModelledCalibrated => "modelled_calibrated",
        EvidenceClass::ModelledUncalibrated => "modelled_uncalibrated",
        EvidenceClass::Elicited => "elicited",
        EvidenceClass::Transferred { .. } => "transferred",
        EvidenceClass::Assumed => "assumed",
        EvidenceClass::Assertion { .. } => "assertion",
    }
}

fn render_matdb_manifest(
    corpus_id: &str,
    pack_version: u32,
    entries: &BTreeMap<String, MaterialEntry>,
) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"schema\":\"{}\",\"corpus_id\":\"{}\",\"pack_version\":{},\"sealed\":true,\"entries\":[",
        MATDB_MANIFEST_SCHEMA,
        escape_json(corpus_id),
        pack_version,
    );
    for (e_idx, entry) in entries.values().enumerate() {
        if e_idx > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"material_id\":\"{}\",\"class\":\"{}\",\"evidence\":\"{}\",\"citation\":\"{}\",\"provenance\":[",
            escape_json(&entry.material_id),
            entry.class.as_str(),
            evidence_label(entry.evidence),
            escape_json(&entry.citation),
        );
        for (a_idx, a) in entry.provenance.iter().enumerate() {
            if a_idx > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"path\":\"{}\",\"sha256\":\"{}\",\"locator\":\"{}\"}}",
                escape_json(&a.artifact_path),
                hex32(&a.artifact_sha256),
                escape_json(&a.locator),
            );
        }
        out.push_str("],\"params\":[");
        for (p_idx, p) in entry.params.values().enumerate() {
            if p_idx > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"key\":\"{}\",\"unit\":\"{}\",\"lo\":\"{}/{}\",\"nominal\":\"{}/{}\",\"hi\":\"{}/{}\"}}",
                escape_json(&p.property_key),
                escape_json(&p.unit),
                p.lo.num(),
                p.lo.den(),
                p.nominal.num(),
                p.nominal.den(),
                p.hi.num(),
                p.hi.den(),
            );
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// Seal a collection of validated [`MaterialEntry`] records into a canonical, content-addressed
/// `frankensim.matdb-manifest.v1` [`SealedCorpus`] (`BD-241`, §13.10).
pub fn seal_corpus(
    corpus_id: impl Into<String>,
    pack_version: u32,
    entries: Vec<MaterialEntry>,
) -> Result<SealedCorpus, MaterialsError> {
    let corpus_id = corpus_id.into();
    if corpus_id.trim().is_empty() || pack_version == 0 {
        return Err(MaterialsError::InvalidSpec {
            field: "corpus_id",
            detail: "corpus_id must be non-empty and pack_version >= 1".to_string(),
        });
    }
    if entries.is_empty() {
        return Err(MaterialsError::InvalidSpec {
            field: "entries",
            detail: "sealed corpus requires >= 1 MaterialEntry".to_string(),
        });
    }
    let mut map = BTreeMap::new();
    for entry in entries {
        if entry.material_id.trim().is_empty() || entry.citation.trim().is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "material_entry",
                detail: "material_id and citation must be non-empty".to_string(),
            });
        }
        if entry.provenance.is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "provenance",
                detail: format!(
                    "material '{}' requires >= 1 SourceAnchor",
                    entry.material_id
                ),
            });
        }
        for anchor in &entry.provenance {
            if anchor.artifact_path.trim().is_empty() || anchor.locator.trim().is_empty() {
                return Err(MaterialsError::InvalidSpec {
                    field: "provenance.anchor",
                    detail: format!(
                        "material '{}' has empty artifact_path or locator",
                        entry.material_id
                    ),
                });
            }
        }
        if entry.params.is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "params",
                detail: format!(
                    "material '{}' requires >= 1 MaterialParam",
                    entry.material_id
                ),
            });
        }
        for (k, p) in &entry.params {
            if k != &p.property_key {
                return Err(MaterialsError::InvalidSpec {
                    field: "params",
                    detail: format!(
                        "material '{}' map key '{k}' != property_key '{}'",
                        entry.material_id, p.property_key
                    ),
                });
            }
            let _ = MaterialParam::new(
                p.property_key.clone(),
                p.unit.clone(),
                p.lo,
                p.nominal,
                p.hi,
            )?;
        }
        if map.insert(entry.material_id.clone(), entry).is_some() {
            return Err(MaterialsError::InvalidSpec {
                field: "material_id",
                detail: format!("duplicate material_id in corpus '{corpus_id}'"),
            });
        }
    }
    let manifest_json = render_matdb_manifest(&corpus_id, pack_version, &map);
    let corpus_seal = sha256(manifest_json.as_bytes());
    Ok(SealedCorpus {
        corpus_id,
        pack_version,
        entries: map,
        manifest_json,
        corpus_seal,
    })
}

impl SealedCorpus {
    /// Canonical `frankensim.matdb-manifest.v1` JSON representation.
    #[must_use]
    pub fn manifest_json(&self) -> &str {
        &self.manifest_json
    }

    /// SHA-256 content-address seal over the canonical manifest JSON bytes.
    #[must_use]
    pub const fn seal_digest(&self) -> Digest32 {
        self.corpus_seal
    }

    /// Number of sealed material entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the corpus contains zero entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Verify that the stored manifest and entries match `expected_seal`, refusing any change with
    /// [`MaterialsError::CorpusChanged`] (`FMINE-REF-505`, §13.10).
    pub fn verify_seal(&self, expected_seal: Digest32) -> Result<(), MaterialsError> {
        let recomputed_json =
            render_matdb_manifest(&self.corpus_id, self.pack_version, &self.entries);
        let recomputed_digest = sha256(recomputed_json.as_bytes());
        if recomputed_json != self.manifest_json
            || recomputed_digest != self.corpus_seal
            || recomputed_digest != expected_seal
        {
            return Err(MaterialsError::CorpusChanged {
                corpus_id: self.corpus_id.clone(),
                expected_hex: expected_seal.to_hex(),
                actual_hex: recomputed_digest.to_hex(),
            });
        }
        Ok(())
    }

    /// Verify external `manifest_bytes` against this corpus's seal (`StoreError::CorpusChanged`).
    pub fn verify_manifest_bytes(&self, manifest_bytes: &[u8]) -> Result<(), MaterialsError> {
        let actual = sha256(manifest_bytes);
        if actual != self.corpus_seal {
            return Err(MaterialsError::CorpusChanged {
                corpus_id: self.corpus_id.clone(),
                expected_hex: self.corpus_seal.to_hex(),
                actual_hex: actual.to_hex(),
            });
        }
        Ok(())
    }

    /// Look up one parameter from the sealed corpus under `reader` confidentiality clearance.
    ///
    /// - Refuses absent material/property with `FMINE-REF-201` (`NoClaimInDomain`).
    /// - Refuses insufficient clearance with `FMINE-REF-301` (`MnpiWall301`, `INV-007`).
    pub fn lookup_param(
        &self,
        material_id: &str,
        property_key: &str,
        reader: &Confidentiality,
    ) -> Result<&MaterialParam, MaterialsError> {
        let Some(entry) = self.entries.get(material_id) else {
            return Err(MaterialsError::NoClaimInDomain {
                material_id: material_id.to_string(),
                detail: "material_id absent from sealed corpus".to_string(),
            });
        };
        if !entry.confidentiality.leq(reader) {
            return Err(MaterialsError::ConfidentialityDenied {
                material_id: material_id.to_string(),
                detail: "entry confidentiality exceeds reader clearance".to_string(),
            });
        }
        entry
            .params
            .get(property_key)
            .ok_or_else(|| MaterialsError::NoClaimInDomain {
                material_id: material_id.to_string(),
                detail: format!("property '{property_key}' absent from material '{material_id}'"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::LabelRegistry;

    fn sample_anchor(path: &str, loc: &str) -> SourceAnchor {
        SourceAnchor::new(path, sha256(path.as_bytes()).0, loc)
    }

    #[test]
    fn test_sealed_geomaterial_corpus_matdb_manifest_tamper_and_walls() {
        let reg = LabelRegistry::new([], ["W-site-lab-2026"]).expect("label registry");
        let wall = reg.wall("W-site-lab-2026").expect("wall");

        let e_steel = MaterialEntry::build(
            "sag_liner_cr_mo_steel",
            MaterialClass::MillLinerSteel,
            EvidenceClass::Measured,
            Confidentiality::public(),
            vec![sample_anchor("vendor/liner_cert_2026.pdf", "p.2 Table 1")],
            "ISO 4885 Cr-Mo SAG liner spec",
            vec![
                MaterialParam::new(
                    "brinell_hardness",
                    "HBW",
                    ExactQty::from_integer(340),
                    ExactQty::from_integer(360),
                    ExactQty::from_integer(380),
                )
                .unwrap(),
                MaterialParam::new(
                    "density",
                    "t/m3",
                    ExactQty::new(780, 100).unwrap(),
                    ExactQty::new(785, 100).unwrap(),
                    ExactQty::new(790, 100).unwrap(),
                )
                .unwrap(),
            ],
        )
        .expect("liner entry");

        let e_domain = MaterialEntry::build(
            "domain_12_fresh_granodiorite",
            MaterialClass::DomainSummary,
            EvidenceClass::ModelledUncalibrated,
            Confidentiality::mnpi(wall.clone()),
            vec![sample_anchor(
                "strata/l4_projection_d12.fmproj",
                "receipt:d12",
            )],
            "L4 projection receipt hash blake3:d12-fresh-granodiorite",
            vec![
                MaterialParam::new(
                    "ucs",
                    "MPa",
                    ExactQty::from_integer(95),
                    ExactQty::from_integer(132),
                    ExactQty::from_integer(170),
                )
                .unwrap(),
                MaterialParam::new(
                    "bond_wi",
                    "kWh/t",
                    ExactQty::new(145, 10).unwrap(),
                    ExactQty::new(162, 10).unwrap(),
                    ExactQty::new(181, 10).unwrap(),
                )
                .unwrap(),
            ],
        )
        .expect("domain summary entry");

        // Order-independent sealing into canonical frankensim.matdb-manifest.v1 format.
        let c1 = seal_corpus(
            "site-geomaterials-2026",
            8,
            vec![e_steel.clone(), e_domain.clone()],
        )
        .expect("seal c1");
        let c2 = seal_corpus("site-geomaterials-2026", 8, vec![e_domain, e_steel])
            .expect("seal c2 reversed");
        assert_eq!(c1.seal_digest(), c2.seal_digest());
        assert_eq!(c1.manifest_json(), c2.manifest_json());
        assert!(
            c1.manifest_json()
                .contains("\"schema\":\"frankensim.matdb-manifest.v1\"")
        );
        assert!(c1.manifest_json().contains("\"sealed\":true"));
        c1.verify_seal(c1.seal_digest()).expect("seal verifies");

        // Tampering with manifest bytes refuses with CorpusChanged -> FMINE-REF-505.
        let mut tampered = c1.manifest_json().as_bytes().to_vec();
        tampered[10] ^= 0x01;
        let err = c1.verify_manifest_bytes(&tampered).unwrap_err();
        assert!(matches!(err, MaterialsError::CorpusChanged { .. }));
        assert_eq!(err.to_refusal().code, "FMINE-REF-505");

        // Public lookup succeeds on public entry, refuses MNPI entry with FMINE-REF-301,
        // and refuses unknown material/property with FMINE-REF-201.
        let hbw = c1
            .lookup_param(
                "sag_liner_cr_mo_steel",
                "brinell_hardness",
                &Confidentiality::public(),
            )
            .expect("public lookup");
        assert_eq!(hbw.nominal, ExactQty::from_integer(360));

        let wall_err = c1
            .lookup_param(
                "domain_12_fresh_granodiorite",
                "ucs",
                &Confidentiality::public(),
            )
            .unwrap_err();
        assert_eq!(wall_err.to_refusal().code, "FMINE-REF-301");

        let cleared = c1
            .lookup_param(
                "domain_12_fresh_granodiorite",
                "bond_wi",
                &Confidentiality::mnpi(wall),
            )
            .expect("cleared lookup");
        assert_eq!(cleared.nominal, ExactQty::new(162, 10).unwrap());

        let missing_err = c1
            .lookup_param("unregistered_reagent", "purity", &Confidentiality::public())
            .unwrap_err();
        assert_eq!(missing_err.to_refusal().code, "FMINE-REF-201");
    }
}
