# CONTRACT.md — `fmine-time`

## 1. Charter & Layer
- **Crate:** `fmine-time`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `10k planned / 12k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["stamp", "calendar", "quad"]`

## 2. Mathematical & Domain Formulation
Implements `RecordTime`, `ValidTime`, `TimeEvidence`, proleptic Gregorian `CivilDate` (Howard Hinnant algorithm), `SiteCalendar`, `FiscalCalendar`, and `QuadTemporalStamp` (`t_event`, `t_avail`, `t_ingested`, `t_model_cut`) with `AsOfQuery` (`G-TIME-1`, `G-TIME-2`).

## 3. Authority, Epistemics & Representation
Guarantees zero assay-availability lookahead (`t_avail <= as_of_cutoff`) in historical backtests (`INV-016`).

## 4. Public API Surface
- `RecordTime`, `ValidTime`, `TimeEvidence`
- `CivilDate`, `ShiftWindow`, `SiteCalendar`, `FiscalPeriod`, `FiscalCalendar`
- `QuadTemporalStamp`, `AsOfQuery`

## 5. Guards & Typed Refusals
- `FMINE-REF-108` (assay-availability lookahead, `TimeError::AssayLagLookahead`, `G-TIME-2`), the only variant
  `TimeError::refusal_code` maps. It is a string: the crate sits below `fmine-evidence`.

## 6. Proof Obligations (`PO-*`)
- `PO-TIME-1`: Bijective `CivilDate <-> epoch_days` across `1900-01-01..2200-12-31`.
- `PO-TIME-2`: `AsOfQuery::admits` refuses any stamp with `t_avail > as_of_cutoff` (`FMINE-REF-108`).

## 7. Failure Modes & Deterministic Fallbacks
Invalid civil dates, inverted intervals and a shift crossing a fiscal cut without a split rule (`G-TIME-1`) fail
closed with typed `TimeError`s that carry no refusal code.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-TIME-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`.

## 10. Non-Goals & Scope Boundary
Never calls `SystemTime::now()` in pure kernel evaluation (`L-3`).
