# CONTRACT.md — `fmine-materials`

## 1. Purpose
Strata K0 sealed geomaterial corpus and L3 empirical correlation card kernel (`BD-241`, plan §13.10):
- `corpus`: non-spatial material classes (mill-liner/GET steels, grinding media, reagents, process water, CRMs/reference ores, TSF construction zones, geomembranes, explosives, and domain summary projections) sealed into content-addressed `frankensim.matdb-manifest.v1` bundles (`seal_corpus`, `verify_seal`, `CorpusChanged` tamper refusal, per-claim `SourceAnchor` + `Confidentiality`, and `NoClaimInDomain` refusal outside the corpus);
- `cards`: L3 empirical correlation cards (`UCS(Is50)`, `density(S, Fe)`, `BondWi`, `PRI(TOC)`, `recovery(CuAS/CuT)`, `ArchardWear`) carrying exact `ExactQty` validity boxes `[x_min, x_max]`, exact rational coefficients, and evidence classes capped at `ModelledCalibrated` or `ModelledUncalibrated` (never promoted by verification, plan §13.10).

- **Crate:** `fmine-materials`
- **Layer:** K0
- **Subsystem:** Strata
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["corpus", "cards"]`

## 2. Inputs
- Material entry specs (`MaterialEntry`, `MaterialParam`) carrying exact `fmine_units::ExactQty` intervals `[lo, hi]`, explicit unit strings, `SourceAnchor` provenance, and `fmine_claim::Confidentiality` labels (zero bare `f64` parameters in public K-layer signatures; L-6).
- L3 correlation card specs (`LinearCorrelationCard`) carrying exact `ExactQty` slope, intercept, and validity box `[min_x, max_x]`.
- Dependencies: `fmine-core`, `fmine-units`, `fmine-evidence`, `fmine-claim` (`K0 → S0`, zero external crates, zero I/O/clocks/threads; L-3).

## 3. Outputs
- Immutable `SealedCorpus` carrying its canonical `frankensim.matdb-manifest.v1` JSON manifest bytes and SHA-256 `corpus_seal` digest, checked lookups respecting reader `Confidentiality`, and `CorrelationEval` outputs carrying exact `ExactQty` intervals and capped `EvidenceClass`.
- Structured `MaterialsError` mapped to registered `FMINE-REF-*` refusal codes.

## 4. Invariants
- **Sealed private corpora only (`BD-241`, §13.10):** every corpus is sealed with a canonical SHA-256 digest over its sorted `frankensim.matdb-manifest.v1` payload; any post-seal mutation refuses with `MaterialsError::CorpusChanged` (`FMINE-REF-505`).
- **Confidentiality wall enforcement (`INV-007`):** reading or exporting an entry whose `Confidentiality` exceeds the reader's clearance refuses with `FMINE-REF-301` (`MnpiWall301`).
- **No silent extrapolation (`G-AOA`):** querying an absent material/property refuses with `FMINE-REF-201` (`NoClaimInDomain201`), and evaluating a correlation card outside its exact `[min_x, max_x]` validity box refuses with `FMINE-REF-203` (`OutsideAoa203`).
- **No evidence promotion (`INV-006`, §13.10):** L3 correlation cards reject `EvidenceClass::Measured` or `Derived` claims and never promote `ModelledUncalibrated` outputs via verification.

## 5. Refusals
- `MaterialsError::CorpusChanged` → `FMINE-REF-505` (exit 6): sealed corpus manifest bytes or seal digest tampered (`StoreError::CorpusChanged`).
- `MaterialsError::NoClaimInDomain` → `FMINE-REF-201` (exit 3): material id or property key absent from the sealed corpus.
- `MaterialsError::OutsideValidityBox` → `FMINE-REF-203` (exit 3): correlation card input lies outside `[min_x, max_x]`.
- `MaterialsError::ConfidentialityDenied` → `FMINE-REF-301` (exit 4): reader lacks clearance for entry's confidentiality label.
- `MaterialsError::InvalidSpec` / `MaterialsError::Overflow` → `FMINE-REF-109` (exit 3): invalid bounds (`lo > hi`), duplicate ids, missing provenance, or exact rational overflow.
- `MaterialsError::UnitMismatch` → `FMINE-REF-105` (exit 3): correlation card input unit differs from declared input unit.

## 6. Determinism class
All files (`lib.rs`, `corpus.rs`, `cards.rs`) declare `//! Determinism: BitwisePortable`. Arithmetic uses exact `i128` rationals (`ExactQty`) and canonical `BTreeMap` ordering for deterministic `frankensim.matdb-manifest.v1` serialization.

## 7. Evidence class of outputs
Non-spatial corpus lookups preserve each entry's declared `EvidenceClass` and `SourceAnchor` provenance without upgrading; L3 correlation cards emit strictly `ModelledCalibrated` (when a calibration record id is bound) or `ModelledUncalibrated` (`Estimated`).

## 8. Validity domain
- Exact rational intervals within `i128` `ExactQty` overflow limits.
- `frankensim.matdb-manifest.v1` JSON export covers the FrankenMining sealed private corpus schema; upstream frankensim pack files are never modified (§13.10).

## 9. Oracles
- Known-answer sealing, canonical JSON round-trip, tamper refusal (`CorpusChanged`), confidentiality wall refusal (`FMINE-REF-301`), out-of-domain refusal (`FMINE-REF-201`), and exact L3 correlation card validity-box tests (`corpus::tests::test_sealed_geomaterial_corpus_matdb_manifest_tamper_and_walls`, `cards::tests::test_l3_correlation_cards_validity_box_and_no_promotion`).

## 10. Known gaps
- Filesystem bundle storage (`fs-matdb-store` adapter) and live L0→L4 block-domain summary projection pipelines are deferred to the `V`-layer storage and `fmine-belief` crates.

## 11. Public API
- `MaterialsError`
- `corpus::{MATDB_MANIFEST_SCHEMA, MaterialClass, MaterialParam, MaterialEntry, SealedCorpus, seal_corpus}` (`BD-241`)
- `cards::{CorrelationKind, ValidityBox, LinearCorrelationCard, CorrelationEval}` (`BD-241`, §13.10)

## 12. Proof obligations
- `BD-241` (sealed geomaterial corpus in `frankensim.matdb-manifest.v1` format, `CorpusChanged` tamper refusal, confidentiality & no-claim boundaries): `corpus::tests::test_sealed_geomaterial_corpus_matdb_manifest_tamper_and_walls`.
- `BD-241` / §13.10 (L3 empirical correlation cards, validity-box refusal, no evidence promotion): `cards::tests::test_l3_correlation_cards_validity_box_and_no_promotion`.
