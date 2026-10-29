# CONTRACT.md — `fmine-public`

## 1. Charter & Layer
- **Crate:** `fmine-public`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Assurance`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["corpus", "bridge", "assurance"]`

## 2. Mathematical & Domain Formulation
Implements the `BARRICK_18_WORKBOOKS` public source registry (`BD-055`), `cell_to_public_claim` (`BD-058`), `BarrickCorpus::load_from_workspace` (`BD-059`), `extract_barrick_gold_aisc_bridge`, and the end-to-end M0 Public Assurance Demonstrator (`run_barrick_m0_assurance`, `PublicAssuranceReport`, `DiscrepancyRegister`, `GATE-BULL-01`, `BD-066`).

## 3. Authority, Epistemics & Representation
Binds every public-filing claim and discrepancy entry to an exact `SourceAnchor` and commits the corpus verification state to an `FMT-ANCHOR-1` `LedgerAnchor` (`CELL-PUBLIC(barrick)`).

## 4. Public API Surface
- `FilingKind`, `FilingDescriptor`, `BARRICK_18_WORKBOOKS`
- `cell_to_public_claim`, `LoadedFiling`, `BarrickCorpus`
- `extract_barrick_gold_aisc_bridge`
- `DiscrepancyCategory`, `DiscrepancyEntry`, `PublicAssuranceReport`, `run_barrick_m0_assurance`

## 5. Guards & Typed Refusals
- `FMINE-REF-401` on formula-error or unanchored public cells.
- `FMINE-REF-203` (`ConsolidationBridgeMissing`, `G-BULL-3`) via `AiscConsolidationBridge::verify_rollup`.

## 6. Proof Obligations (`PO-*`)
- `PO-PUB-1`: `run_barrick_m0_assurance` verifies all 18 Barrick `.xlsx` workbooks (`37,108` cell anchors), reconciles FY2024 (`L_corp = -$49M`) and FY2025 Gold AISC bridges, decomposes North Mara `2024Q2` `+$1,285/oz` spike with `0.0` residual, and populates `DiscrepancyRegister`.

## 7. Failure Modes & Deterministic Fallbacks
Missing anchor labels or corrupted workbooks fail closed with `IoError`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-io`, `fmine-metrics`.

## 10. Non-Goals & Scope Boundary
Operates strictly on `ConfidentialityTier::Public` disclosures (`CELL-PUBLIC`).
