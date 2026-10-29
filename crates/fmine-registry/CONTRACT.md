# CONTRACT.md — `fmine-registry`

## 1. Charter & Layer
- **Crate:** `fmine-registry`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `10k planned / 12k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["catalog", "parser"]`

## 2. Mathematical & Domain Formulation
Parses and validates the 8 canonical TOML registries in `registries/` (`loc_budget`, `workspace_topology`, `lifts`, `units`, `crs`, `calendars`, `refusals`, `verbs`) and verifies 1:1 alignment between `registries/refusals.toml` and `fmine_evidence::REFUSAL_CATALOG`.

## 3. Authority, Epistemics & Representation
Serves as the schema and registry catalog authority for `fmine doctor` and `xtask`.

## 4. Public API Surface
- `RegistryCatalog`, `RegistryError`

## 5. Guards & Typed Refusals
- `RegistryError::MissingEntry`, `RegistryError::CatalogMismatch`, `RegistryError::Io`.

## 6. Proof Obligations (`PO-*`)
- `PO-REG-1`: `RegistryCatalog::load_from_root` verifies all 8 workspace TOML registries and 100% refusal-code parity with `REFUSAL_CATALOG`.

## 7. Failure Modes & Deterministic Fallbacks
Missing or out-of-sync registry entries fail closed with `RegistryError`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-LAYERS-00`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Read-only TOML validation; never mutates registry files at runtime.
