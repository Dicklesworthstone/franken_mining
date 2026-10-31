# CONTRACT.md — `fmine-time`

## 1. Charter & Layer
- **Crate:** `fmine-time`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `10k planned / 12k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["stamp", "calendar", "quad"]`

## 2. Mathematical & Domain Formulation
Implements `RecordTime`, `ValidTime`, `TimeEvidence`, proleptic Gregorian `CivilDate` (Howard Hinnant algorithm), `SiteCalendar`, `FiscalCalendar`, and `QuadTemporalStamp` with `AsOfQuery` (`G-TIME-1`, `G-TIME-2`).

`QuadTemporalStamp` holds the four times of plan §6.3, private so that no stamp can hold an impossible order:
- `physical_valid_time`: the interval over which the physical state held;
- `collection_time`: when the sample, survey pickup or sensor frame was taken;
- `availability_time`: when the certificate, QA/QC disposition or processed reading became visible to a site
  decision-maker;
- `record_time`: when the ledger committed it. Here a `RecordTime`, not yet the plan's `(cell, seq)` under an anchor.

A decision time is a property of the query, not of the observation, so it lives on `AsOfQuery`. `availability_time`
and `record_time` may each equal `collection_time`, and either may come first: a ledger can commit a sample before
its certificate arrives.

## 3. Authority, Epistemics & Representation
Guarantees zero availability lookahead (`availability_time <= t`) in historical backtests (`INV-016`):
`QuadTemporalStamp::as_of(t)` is `availability_time ≤ t ∧ record_time ≤ t`, and it is monotone in `t`.

## 4. Public API Surface
- `RecordTime`, `ValidTime`, `TimeEvidence`
- `CivilDate`, `ShiftWindow`, `SiteCalendar`, `FiscalPeriod`, `FiscalCalendar`
- `QuadTemporalStamp`, `AsOfQuery`

## 5. Guards & Typed Refusals
- `FMINE-REF-108` (assay-availability lookahead, `TimeError::AssayLagLookahead`, `G-TIME-2`), the only variant
  `TimeError::refusal_code` maps. It is a string: the crate sits below `fmine-evidence`.
- `TimeError::StampOrder`: `QuadTemporalStamp::new` refuses `availability_time` or `record_time` before
  `collection_time`. Nothing is reordered or clamped. Like `InvalidInterval`, it has no `FMINE-REF` code.

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
