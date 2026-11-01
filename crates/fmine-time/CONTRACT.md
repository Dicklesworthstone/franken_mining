# CONTRACT.md — `fmine-time`

## 1. Purpose
Time for S0. It provides:
- record time as a ledger commit position (`RecordTime`);
- valid-time intervals;
- the proleptic Gregorian civil calendar with ISO weekday and week (Hinnant's algorithm);
- one IANA zone's offsets over a span (`ZoneRules`), plus site and fiscal calendars;
- the four-time `QuadTemporalStamp` of plan §6.3, with an `AsOfQuery` that refuses hindsight.

Callers: fmine-calib, fmine-claim, fmine-cli, fmine-decide, fmine-registry and xtask; fmine-metrics and fmine-public declare the dependency but do not use it. It never reads the wall clock (`SystemTime::now`, L-3). It does not resolve cross-cell digests; that is BD-108's gluing checker.

- **Crate:** `fmine-time`
- **Layer:** S0
- **Subsystem:** Bedrock
- **LOC budget:** 8,000 planned / 9,000 ceiling
- **Allowed Module Families:** `["stamp", "calendar", "quad"]`

## 2. Inputs
- Civil dates, or days since 1970-01-01.
- Nanosecond instants and intervals.
- `(cell, seq)` commit positions, with the wall-clock reading at commit and its `TimeEvidence`.
- `Anchor` high-water marks, built directly or from one `fmine_core::LedgerAnchor` per cell.
- Zone transitions `(at_utc_s, offset_minutes)` over `[from_utc_s, to_utc_s)`, which `fmine_registry::calendars` builds from `registries/zones.toml`.
- Production-day start hours and shift lengths.

## 3. Outputs
`CivilDate` values with their epoch days, ISO weekday and ISO week; UTC offsets, and UTC instants for local times; production-day cuts in UTC (`site_day_cut_utc_ns`); `QuadTemporalStamp`; visibility verdicts from `as_of` and `AsOfQuery::check_admissible`. None is a Claim.

## 4. Invariants
- INV-012, records ordered by commit and never by wall clock. `RecordTime::commit_order` orders commits of one cell by `seq` and gives `None` across cells, and the anchor alone decides cross-cell visibility. Tests: `test_record_order_is_by_seq_within_a_cell_and_none_across_cells`, `test_cross_cell_visibility_follows_the_anchor_not_the_wall_clock`.
- INV-016, no availability lookahead. `as_of(t, anchor)` holds when `availability_time ≤ t` and the anchor holds `record_time`, and it is monotone in `t`. Tests: `test_quad_temporal_assay_lag_guard_g_time_2`, `test_fire_assay_lag_of_3_to_21_days_is_refused_until_available`, `test_late_reading_is_invisible_until_available_and_committed`, `test_as_of_is_monotone_in_t`.
- The four times stay separate, and no stamp holds an impossible order: `test_stamp_keeps_the_four_times_apart`, `test_stamp_admits_no_lag_and_a_record_before_availability`.

## 5. Refusals
- `TimeError::AssayLagLookahead` (G-TIME-2) → FMINE-REF-108, assay-availability lookahead, exit 3 (`src/lib.rs:63`). This is the only variant `refusal_code` maps, and it is a string because the crate sits below `fmine-evidence`.
- The other typed errors carry no code:
  - `StampOrder`: availability, or the commit's wall-clock reading, before collection. Tests: `test_stamp_refuses_availability_before_collection`, `test_stamp_refuses_record_before_collection`.
  - `CellNotInAnchor`: reported upstream as not observable. Test: `test_cell_missing_from_the_anchor_is_not_observable`.
  - `AnchorCellRepeated`: never resolved by max or min. Test: `test_anchor_reads_ledger_anchors_and_refuses_a_cell_named_twice`.
  - `ZoneSnapshot`: an empty zone name, an empty or out-of-range span, an offset beyond ±18 h, or transitions that do not increase, leave the span or keep the offset. Test: `test_zone_rules_refuse_malformed_snapshots`.
  - `OutsideZoneSnapshot`: an instant, or a local time's candidates, outside the span; never extrapolated.
  - `LocalTimeNonexistent` (a skipped local time) and `LocalTimeAmbiguous` (a repeated one): no side is picked. Test: `test_site_day_cut_refuses_a_skipped_or_repeated_start_and_hour_24`.
  - `InvalidCalendar`, `InvalidInterval`, `InvalidCivilDate`.
  - `CalendarCutMismatch` (G-TIME-1): the site and fiscal cuts differ and pro-rata is off, or the fiscal cut falls outside the day's first shift; it is refused, not clamped to 0 or 1. Test: `test_pro_rata_refuses_a_fiscal_cut_outside_the_first_shift`.

Nothing is reordered, sorted, merged or clamped.

## 6. Determinism class
Undeclared: `src/lib.rs` has no `//! Determinism:` header, which `xtask check-det` requires only in the K and E layers. The calendar and zone code is integer arithmetic.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted. `TimeEvidence` records how a commit's wall-clock reading was obtained; it is not a §7.1 class.

## 8. Validity domain
- Civil dates are checked against the oracle from 1900-01-01 to 2200-12-31.
- Zone offsets exist only inside the snapshot's span, and the committed snapshot runs 1970–2100.
- Offsets are within ±18 h, and a production day starts before hour 24.
- Local wall-clock times are seconds from 1970-01-01T00:00 local.

## 9. Oracles
- `fixtures/fmine-time/civil_iso_1900_2200.txt`: CPython `datetime`, 16 dates a year, made by `scripts/oracle/civil_iso_table.py`, with its capture record beside it. Test: `test_po_time_1_calendar_matches_the_pinned_oracle_table`.
- `fixtures/fmine-time/zone_oracle_1970_2100.txt`: CPython `zoneinfo` over tzdata 2026e, made by `scripts/oracle/zone_snapshot.py`, with its capture record. Its test, `test_po_time_3_zone_snapshot_matches_the_pinned_oracle_table`, lives in fmine-registry, which loads the snapshot.

## 10. Known gaps
- BD-028 (bd-028), BD-029 (bd-029) and BD-309 (bd-309): the code and tests exist, but each stays open behind its blockers. bd-028 waits on bd-016 (the GATE-LAYERS-00 chain); bd-029 waits on bd-028; bd-309 waits on bd-028, bd-032 and bd-g507.
- NEW (bd-g507): the four-field stamp needs verification by someone other than its author.
- No FMINE-REF code exists for the stamp-order, anchor and zone refusals (BD-328 owns the registry).

## 11. Public API
- `TimeError` (`refusal_code`), `TimeEvidence`, `RecordTime` (`commit_order`), `Anchor` (`from_ledger_anchors`, `mark`, `has_committed`), `ValidTime` (`overlaps`, `contained_in`)
- `CivilDate` (`to_epoch_days`, `from_epoch_days`, `iso_weekday`, `iso_week`), `is_leap_year`, `days_in_month`, and the `NS_PER_*` and `SECS_PER_DAY` constants
- `ZoneRules` (`offset_minutes_at`, `local_to_utc_s`), `SiteCalendar` (`site_day_cut_utc_ns`), `FiscalCalendar` (`verify_fiscal_alignment`)
- `QuadTemporalStamp` (`new`, `as_of`), `QueryPurpose`, `AsOfQuery` (`check_admissible`)

## 12. Proof obligations
- PO-TIME-1 (civil dates biject with epoch days from 1900 to 2200, and agree with the oracle): `test_civil_calendar_roundtrip_1900_to_2200`, `test_po_time_1_calendar_matches_the_pinned_oracle_table`.
- PO-TIME-2 (an operational query refuses a stamp available after its cut-off, with FMINE-REF-108): `test_quad_temporal_assay_lag_guard_g_time_2`. The plan names the method `AsOfQuery::admits`; the code's is `check_admissible`, and a `RetrospectiveAccounting` query admits the stamp once the anchor holds it.
- PO-TIME-3 (offsets, local times and day cuts agree with the zone oracle): `test_po_time_3_zone_snapshot_matches_the_pinned_oracle_table`, `test_site_day_cut_follows_daylight_saving`, `test_zone_offset_holds_from_each_transition_second`.
