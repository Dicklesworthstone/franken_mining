//! Quad-temporal timestamps (`physical_valid_time`, `collection_time`, `availability_time`, `record_time`; plan §6.3),
//! civil calendar arithmetic, site/fiscal calendars (`G-TIME-1`), and assay-lag lookahead
//! guards (`G-TIME-2`, `FMINE-REF-108`).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

use fmine_core::LedgerAnchor;

/// Errors raised by temporal interval, calendar alignment, or assay-lag lookahead guards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeError {
    /// `end_ns < start_ns` in a `ValidTime` interval.
    InvalidInterval { start_ns: i64, end_ns: i64 },
    /// Invalid civil year/month/day.
    InvalidCivilDate { year: i32, month: u8, day: u8 },
    /// `G-TIME-1`: Site shift window crosses fiscal cut-off without an explicit split rule.
    CalendarCutMismatch {
        site_id: String,
        shift_end_utc_ns: i64,
        fiscal_cut_utc_ns: i64,
    },
    /// `FMINE-REF-108` (`G-TIME-2`): Assay or observation was not yet reported at `as_of_decision`.
    AssayLagLookahead {
        availability_time: i64,
        as_of_decision_ns: i64,
    },
    /// A `QuadTemporalStamp` instant before its `collection_time`: impossible, so refused, never reordered.
    StampOrder {
        field: &'static str,
        at_ns: i64,
        collection_time: i64,
    },
    /// `INV-012`: the anchor has no high-water mark for the record's cell, so the record is not observable at it.
    /// It is neither counted as absent nor as present; the consumer reports `not_observable`.
    CellNotInAnchor { cell: String, seq: u64 },
    /// An anchor that names a cell twice has no one high-water mark for it: refused, never resolved by max or min.
    AnchorCellRepeated { cell: String },
}

impl TimeError {
    /// Returns the canonical `FMINE-REF-*` refusal code when applicable.
    #[must_use]
    pub const fn refusal_code(&self) -> Option<&'static str> {
        match self {
            Self::AssayLagLookahead { .. } => Some("FMINE-REF-108"),
            _ => None,
        }
    }
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterval { start_ns, end_ns } => {
                write!(f, "invalid time interval [{start_ns}, {end_ns}]")
            }
            Self::InvalidCivilDate { year, month, day } => {
                write!(f, "invalid civil date {year:04}-{month:02}-{day:02}")
            }
            Self::CalendarCutMismatch {
                site_id,
                shift_end_utc_ns,
                fiscal_cut_utc_ns,
            } => write!(
                f,
                "G-TIME-1: site '{site_id}' shift end {shift_end_utc_ns} != fiscal cut {fiscal_cut_utc_ns}"
            ),
            Self::AssayLagLookahead {
                availability_time,
                as_of_decision_ns,
            } => write!(
                f,
                "FMINE-REF-108: available at {availability_time} > decision time {as_of_decision_ns}"
            ),
            Self::StampOrder {
                field,
                at_ns,
                collection_time,
            } => write!(
                f,
                "{field} {at_ns} is before collection_time {collection_time}"
            ),
            Self::CellNotInAnchor { cell, seq } => write!(
                f,
                "record {seq} of cell {cell} is not observable: the anchor has no mark for {cell}"
            ),
            Self::AnchorCellRepeated { cell } => {
                write!(f, "the anchor names cell {cell} more than once")
            }
        }
    }
}

impl std::error::Error for TimeError {}

/// Provenance quality of a clock reading (`BD-028`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeEvidence {
    /// Synchronized host clock (`PTP`/`GPS` disciplined).
    HostSynced,
    /// NTP-bounded clock with bounded maximum skew in milliseconds.
    NtpBounded { max_skew_ms: u32 },
    /// Offline edge device clock with uncertain drift.
    OfflineUncertain,
}

/// When the ledger committed a record: commit `seq` of authority cell `cell` (plan §6.3, §25.4, `BD-028`), with the
/// wall-clock reading taken at commit and its [`TimeEvidence`].
///
/// Commits of one cell are ordered by `seq`. Records of two cells have no order of their own, whatever their wall
/// clocks say: only an [`Anchor`] relates them (`INV-012`). So the type has no `Ord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTime {
    cell: String,
    seq: u64,
    wall_ns: i64,
    evidence: TimeEvidence,
}

impl RecordTime {
    /// Commit `seq` of `cell`, read at `wall_ns` (`UTC` nanoseconds since Unix epoch) on a clock of quality `evidence`.
    #[must_use]
    pub fn new(cell: impl Into<String>, seq: u64, wall_ns: i64, evidence: TimeEvidence) -> Self {
        Self {
            cell: cell.into(),
            seq,
            wall_ns,
            evidence,
        }
    }

    /// The authority cell that committed the record.
    #[must_use]
    pub fn cell(&self) -> &str {
        &self.cell
    }

    /// The record's commit sequence number in its cell.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The wall-clock reading at commit. It is shown and checked against `collection_time`; it never orders records.
    #[must_use]
    pub const fn wall_ns(&self) -> i64 {
        self.wall_ns
    }

    /// The quality of the clock that gave [`Self::wall_ns`].
    #[must_use]
    pub const fn evidence(&self) -> TimeEvidence {
        self.evidence
    }

    /// Commit order: by `seq` within one cell, and `None` across cells.
    #[must_use]
    pub fn commit_order(&self, other: &Self) -> Option<Ordering> {
        (self.cell == other.cell).then(|| self.seq.cmp(&other.seq))
    }
}

/// The anchor of plan §6.3: for each authority cell a consumer has synchronized, the highest commit `seq` it holds.
/// It is the only thing that relates records of different cells (`INV-012`). Digests are checked when a cross-cell
/// reference is resolved (`BD-108`), not here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Anchor {
    marks: BTreeMap<String, u64>,
}

impl Anchor {
    /// An anchor with one high-water mark per cell. Refuses a cell named twice ([`TimeError::AnchorCellRepeated`]).
    pub fn new<C: Into<String>>(
        marks: impl IntoIterator<Item = (C, u64)>,
    ) -> Result<Self, TimeError> {
        let mut map = BTreeMap::new();
        for (cell, seq) in marks {
            match map.entry(cell.into()) {
                Entry::Occupied(taken) => {
                    return Err(TimeError::AnchorCellRepeated {
                        cell: taken.key().clone(),
                    });
                }
                Entry::Vacant(slot) => {
                    slot.insert(seq);
                }
            }
        }
        Ok(Self { marks: map })
    }

    /// The anchor that a set of per-cell [`LedgerAnchor`]s names: each one's `cell_id` at its `commit_sequence`.
    pub fn from_ledger_anchors(anchors: &[LedgerAnchor]) -> Result<Self, TimeError> {
        Self::new(
            anchors
                .iter()
                .map(|a| (a.cell_id.as_str(), a.commit_sequence)),
        )
    }

    /// The high-water mark of `cell`, if the anchor has one.
    #[must_use]
    pub fn mark(&self, cell: &str) -> Option<u64> {
        self.marks.get(cell).copied()
    }

    /// Whether the anchor holds `record`: `true` when its cell's mark is at or past its `seq`, `false` when not yet.
    /// A cell the anchor does not name is refused ([`TimeError::CellNotInAnchor`], `INV-012`).
    pub fn has_committed(&self, record: &RecordTime) -> Result<bool, TimeError> {
        self.mark(&record.cell)
            .map(|mark| record.seq <= mark)
            .ok_or_else(|| TimeError::CellNotInAnchor {
                cell: record.cell.clone(),
                seq: record.seq,
            })
    }
}

/// Physical validity interval `[start_ns, end_ns]` (`UTC` nanoseconds since Unix epoch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidTime {
    pub start_ns: i64,
    pub end_ns: i64,
}

impl ValidTime {
    /// Constructs a validated interval `[start_ns, end_ns]` with `start_ns <= end_ns`.
    pub fn new(start_ns: i64, end_ns: i64) -> Result<Self, TimeError> {
        if end_ns < start_ns {
            return Err(TimeError::InvalidInterval { start_ns, end_ns });
        }
        Ok(Self { start_ns, end_ns })
    }

    /// Instantaneous valid time `[t, t]`.
    #[must_use]
    pub const fn instant(utc_ns: i64) -> Self {
        Self {
            start_ns: utc_ns,
            end_ns: utc_ns,
        }
    }

    /// Returns `true` if `self` overlaps `other`.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.start_ns <= other.end_ns && other.start_ns <= self.end_ns
    }

    /// Returns `true` if `self` is wholly contained within `period`.
    #[must_use]
    pub const fn contained_in(self, period: Self) -> bool {
        self.start_ns >= period.start_ns && self.end_ns <= period.end_ns
    }
}

// --- Proleptic Gregorian Civil Calendar (Howard Hinnant algorithm, BD-028) ---

/// Proleptic Gregorian calendar date (`year`, `month` 1..=12, `day` 1..=31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CivilDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl CivilDate {
    /// Validates and constructs a `CivilDate`.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, TimeError> {
        if !(1..=12).contains(&month) {
            return Err(TimeError::InvalidCivilDate { year, month, day });
        }
        let max_day = days_in_month(year, month);
        if day < 1 || day > max_day {
            return Err(TimeError::InvalidCivilDate { year, month, day });
        }
        Ok(Self { year, month, day })
    }

    /// Converts to days since Unix epoch (`1970-01-01 = 0`).
    #[must_use]
    pub const fn to_epoch_days(self) -> i64 {
        let y = self.year as i64 - if self.month <= 2 { 1 } else { 0 };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = (y - era * 400) as u64;
        let m = self.month as u64;
        let d = self.day as u64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + (doe as i64) - 719_468
    }

    /// Converts days since Unix epoch (`1970-01-01 = 0`) to `CivilDate`.
    #[must_use]
    pub const fn from_epoch_days(z: i64) -> Self {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
        Self {
            year,
            month: m as u8,
            day: d as u8,
        }
    }

    /// ISO 8601 weekday: Monday = 1 through Sunday = 7 (`BD-029`).
    #[must_use]
    pub const fn iso_weekday(self) -> u8 {
        // 1970-01-01 was a Thursday.
        ((self.to_epoch_days() + 3).rem_euclid(7) + 1) as u8
    }

    /// ISO 8601 week-numbering year and week (`1..=53`): the week, Monday to Sunday, that holds the date's Thursday
    /// (`BD-029`). Early January can fall in the previous year's last week, and late December in next year's week 1.
    #[must_use]
    pub const fn iso_week(self) -> (i32, u8) {
        let thursday = self.to_epoch_days() - self.iso_weekday() as i64 + 4;
        let year = Self::from_epoch_days(thursday).year;
        let jan1 = Self {
            year,
            month: 1,
            day: 1,
        }
        .to_epoch_days();
        (year, ((thursday - jan1) / 7 + 1) as u8)
    }
}

/// Returns `true` if `year` is a proleptic Gregorian leap year.
#[must_use]
pub const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in `(year, month)`.
#[must_use]
pub const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

// --- Site & Fiscal Calendars (BD-029, G-TIME-1) ---

/// Nanoseconds per second, minute, hour, and day.
pub const NS_PER_SEC: i64 = 1_000_000_000;
pub const NS_PER_MIN: i64 = 60 * NS_PER_SEC;
pub const NS_PER_HOUR: i64 = 60 * NS_PER_MIN;
pub const NS_PER_DAY: i64 = 24 * NS_PER_HOUR;

/// Site operational shift calendar (`BD-029`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteCalendar {
    pub site_id: String,
    /// Offset from UTC in minutes (e.g. `-480` for Nevada PST, `+300` for Pakistan PKT).
    pub utc_offset_minutes: i32,
    /// Local hour (`0..24`) at which the daily production day starts (e.g., `6` for `06:00`).
    pub day_start_hour_local: u8,
    /// Duration of each shift in hours (`12` for 2x12h, `8` for 3x8h).
    pub shift_hours: u8,
}

/// Corporate fiscal reporting period (`BD-029`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiscalCalendar {
    pub year: i32,
    pub quarter: u8,
    pub period_utc: ValidTime,
}

impl SiteCalendar {
    /// Computes the UTC timestamp (`ns`) of the site's production-day boundary on `date`.
    #[must_use]
    pub fn site_day_cut_utc_ns(&self, date: CivilDate) -> i64 {
        let midnight_utc_ns = date.to_epoch_days() * NS_PER_DAY;
        let local_start_ns = i64::from(self.day_start_hour_local) * NS_PER_HOUR;
        let offset_ns = i64::from(self.utc_offset_minutes) * NS_PER_MIN;
        midnight_utc_ns + local_start_ns - offset_ns
    }

    /// Checks whether the site production cut on `date` matches the fiscal cut-off (`G-TIME-1`).
    /// If `allow_shift_pro_rata` is `false` and the timestamps differ, returns `CalendarCutMismatch`.
    pub fn verify_fiscal_alignment(
        &self,
        date: CivilDate,
        fiscal: FiscalCalendar,
        allow_shift_pro_rata: bool,
    ) -> Result<f64, TimeError> {
        let site_cut = self.site_day_cut_utc_ns(date);
        let fiscal_cut = fiscal.period_utc.end_ns;
        if site_cut == fiscal_cut {
            return Ok(1.0);
        }
        if !allow_shift_pro_rata {
            return Err(TimeError::CalendarCutMismatch {
                site_id: self.site_id.clone(),
                shift_end_utc_ns: site_cut,
                fiscal_cut_utc_ns: fiscal_cut,
            });
        }
        let shift_ns = i64::from(self.shift_hours) * NS_PER_HOUR;
        let delta = (fiscal_cut - site_cut).clamp(0, shift_ns);
        Ok((delta as f64) / (shift_ns as f64))
    }
}

// --- Quad-Temporal Stamp & Assay-Lag Lookahead Guard (BD-032, BD-309, G-TIME-2) ---

/// The four times of one observation (plan §6.3, `BD-309`). A decision time is not one of them: it belongs to the
/// query ([`AsOfQuery`]). The fields are private so that no stamp can hold an impossible order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuadTemporalStamp {
    physical_valid_time: ValidTime,
    collection_time: i64,
    availability_time: i64,
    record_time: RecordTime,
}

impl QuadTemporalStamp {
    /// Builds a stamp. Refuses `availability_time`, or the commit's wall-clock reading, before `collection_time`
    /// ([`TimeError::StampOrder`]); nothing is reordered or clamped.
    pub fn new(
        physical_valid_time: ValidTime,
        collection_time: i64,
        availability_time: i64,
        record_time: RecordTime,
    ) -> Result<Self, TimeError> {
        if availability_time < collection_time {
            return Err(TimeError::StampOrder {
                field: "availability_time",
                at_ns: availability_time,
                collection_time,
            });
        }
        if record_time.wall_ns < collection_time {
            return Err(TimeError::StampOrder {
                field: "record_time",
                at_ns: record_time.wall_ns,
                collection_time,
            });
        }
        Ok(Self {
            physical_valid_time,
            collection_time,
            availability_time,
            record_time,
        })
    }

    /// The interval over which the physical state held.
    #[must_use]
    pub const fn physical_valid_time(&self) -> ValidTime {
        self.physical_valid_time
    }

    /// When the sample, survey pickup or sensor frame was taken.
    #[must_use]
    pub const fn collection_time(&self) -> i64 {
        self.collection_time
    }

    /// When the certificate, QA/QC disposition or processed reading became visible to a site decision-maker.
    #[must_use]
    pub const fn availability_time(&self) -> i64 {
        self.availability_time
    }

    /// Where the ledger committed it.
    #[must_use]
    pub const fn record_time(&self) -> &RecordTime {
        &self.record_time
    }

    /// Whether the record was knowable at `t_ns` under `anchor`: `availability_time ≤ t` and the anchor holds
    /// `record_time` (§6.3, `INV-016`). Refuses a record whose cell the anchor does not name
    /// ([`TimeError::CellNotInAnchor`], `INV-012`).
    pub fn as_of(&self, t_ns: i64, anchor: &Anchor) -> Result<bool, TimeError> {
        Ok(anchor.has_committed(&self.record_time)? && self.availability_time <= t_ns)
    }
}

/// Purpose of an `AsOfQuery` (`BD-309`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryPurpose {
    /// Operational routing / blending / dispatch decision at `as_of_decision_ns`.
    /// Strictly enforces `G-TIME-2` (`availability_time <= as_of_decision_ns`).
    OperationalDecision,
    /// Retrospective period-close accounting / metallurgical balance over `for_physical_period`,
    /// where late-arriving assays for ore mined in `for_physical_period` are admissible provided
    /// the query's anchor holds their `record_time`.
    RetrospectiveAccounting,
}

/// Dual-clock `AS OF` query (`BD-032`, `BD-309`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsOfQuery {
    /// Knowledge cut-off timestamp (`as_of_decision`).
    pub as_of_decision_ns: i64,
    /// The anchor current at `as_of_decision_ns`: which commits of which cells the query may see (§6.3).
    pub anchor: Anchor,
    /// Physical production window being queried (`for_physical_period`).
    pub for_physical_period: ValidTime,
    /// Whether this query drives a real-time routing decision or a retrospective period close.
    pub purpose: QueryPurpose,
}

impl AsOfQuery {
    /// Evaluates whether a record with `stamp` is admissible under this `AsOfQuery`.
    ///
    /// - If `stamp.physical_valid_time()` does not overlap `self.for_physical_period`, returns `Ok(false)`.
    /// - If `self.anchor` does not name the record's cell, returns `Err(TimeError::CellNotInAnchor)` (`INV-012`).
    /// - If `self.anchor` does not yet hold `stamp.record_time()`, returns `Ok(false)`.
    /// - Under `QueryPurpose::OperationalDecision`, if `stamp.availability_time() > self.as_of_decision_ns`,
    ///   returns `Err(TimeError::AssayLagLookahead)` (`FMINE-REF-108`, `G-TIME-2`).
    pub fn check_admissible(&self, stamp: &QuadTemporalStamp) -> Result<bool, TimeError> {
        if !stamp.physical_valid_time.overlaps(self.for_physical_period) {
            return Ok(false);
        }
        if !self.anchor.has_committed(&stamp.record_time)? {
            return Ok(false);
        }
        if self.purpose == QueryPurpose::OperationalDecision
            && stamp.availability_time > self.as_of_decision_ns
        {
            return Err(TimeError::AssayLagLookahead {
                availability_time: stamp.availability_time,
                as_of_decision_ns: self.as_of_decision_ns,
            });
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};

    #[test]
    fn test_civil_calendar_roundtrip_1900_to_2200() {
        let start = CivilDate::new(1900, 1, 1)
            .expect("1900-01-01")
            .to_epoch_days();
        let end = CivilDate::new(2200, 12, 31)
            .expect("2200-12-31")
            .to_epoch_days();
        for d in start..=end {
            let civil = CivilDate::from_epoch_days(d);
            assert_eq!(civil.to_epoch_days(), d, "roundtrip failed at day {d}");
        }
        // Leap year checks: 1900 is not leap, 2000 is leap, 2024 is leap.
        assert!(CivilDate::new(1900, 2, 29).is_err());
        assert!(CivilDate::new(2000, 2, 29).is_ok());
        assert!(CivilDate::new(2024, 2, 29).is_ok());
    }

    /// PO-TIME-1 (BD-029): epoch days, weekday and ISO week agree with the pinned CPython oracle table, 16 dates a
    /// year over 1900-2200 (the table's capture record is beside it in fixtures/fmine-time/).
    #[test]
    fn test_po_time_1_calendar_matches_the_pinned_oracle_table() {
        let table = include_str!("../../../fixtures/fmine-time/civil_iso_1900_2200.txt");
        let mut rows = 0;
        for line in table.lines() {
            let f: Vec<&str> = line.split(' ').collect();
            assert_eq!(f.len(), 5, "{line}");
            let ymd: Vec<i64> = f[0].split('-').map(|p| p.parse().unwrap()).collect();
            let date = CivilDate::new(ymd[0] as i32, ymd[1] as u8, ymd[2] as u8).expect(line);
            let epoch: i64 = f[1].parse().unwrap();
            assert_eq!(date.to_epoch_days(), epoch, "{line}");
            assert_eq!(CivilDate::from_epoch_days(epoch), date, "{line}");
            assert_eq!(date.iso_weekday(), f[2].parse::<u8>().unwrap(), "{line}");
            let week = (f[3].parse::<i32>().unwrap(), f[4].parse::<u8>().unwrap());
            assert_eq!(date.iso_week(), week, "{line}");
            rows += 1;
        }
        assert_eq!(rows, 16 * 301, "16 dates in each year 1900-2200");
        assert!(table.starts_with("1900-01-01 -25567 1 1900 1\n"));
        assert!(table.ends_with("2200-12-31 84370 3 2201 1\n"));
    }

    #[test]
    fn test_site_and_fiscal_calendar_alignment_g_time_1() {
        let carlin = SiteCalendar {
            site_id: "ngm_carlin".to_string(),
            utc_offset_minutes: -480,
            day_start_hour_local: 6,
            shift_hours: 12,
        };
        let q1_end_date = CivilDate::new(2025, 3, 31).expect("valid date");
        let fiscal = FiscalCalendar {
            year: 2025,
            quarter: 1,
            period_utc: ValidTime::new(
                CivilDate::new(2025, 1, 1).unwrap().to_epoch_days() * NS_PER_DAY,
                q1_end_date.to_epoch_days() * NS_PER_DAY + 24 * NS_PER_HOUR,
            )
            .unwrap(),
        };

        // Without explicit shift-split rule, G-TIME-1 refuses unaligned aggregation.
        let err = carlin
            .verify_fiscal_alignment(q1_end_date, fiscal, false)
            .expect_err("must refuse unaligned cut");
        assert!(matches!(err, TimeError::CalendarCutMismatch { .. }));

        // With shift pro-rata enabled, returns deterministic fraction in [0, 1].
        let frac = carlin
            .verify_fiscal_alignment(q1_end_date, fiscal, true)
            .expect("pro-rata allowed");
        assert!((0.0..=1.0).contains(&frac));
    }

    #[test]
    fn test_quad_temporal_assay_lag_guard_g_time_2() {
        // Blast hole drilled at t = 10h, truck routed at t = 12h, fire assay returns at t = 36h.
        let t_blast = 10 * NS_PER_HOUR;
        let t_route = 12 * NS_PER_HOUR;
        let t_assay = 36 * NS_PER_HOUR;

        let stamp = QuadTemporalStamp::new(
            ValidTime::new(t_blast, t_blast + NS_PER_HOUR).unwrap(),
            t_blast,
            t_assay,
            RecordTime::new(SITE, 1, t_blast + NS_PER_HOUR, TimeEvidence::HostSynced),
        )
        .unwrap();

        // 1. Real-time routing query at t_route = 12h MUST refuse the t = 36h assay (FMINE-REF-108).
        let routing_query = AsOfQuery {
            as_of_decision_ns: t_route,
            anchor: site_at(1),
            for_physical_period: ValidTime::new(0, 24 * NS_PER_HOUR).unwrap(),
            purpose: QueryPurpose::OperationalDecision,
        };
        let err = routing_query
            .check_admissible(&stamp)
            .expect_err("must reject lookahead assay");
        assert_eq!(err.refusal_code(), Some("FMINE-REF-108"));

        // 2. Retrospective accounting query at t = 48h for day 1 physical period admits the assay.
        let close_query = AsOfQuery {
            as_of_decision_ns: 48 * NS_PER_HOUR,
            anchor: site_at(1),
            for_physical_period: ValidTime::new(0, 24 * NS_PER_HOUR).unwrap(),
            purpose: QueryPurpose::RetrospectiveAccounting,
        };
        assert!(
            close_query
                .check_admissible(&stamp)
                .expect("admissible at close")
        );

        // 3. The same close at an anchor that does not yet hold the commit leaves it out.
        let early_close = AsOfQuery {
            anchor: site_at(0),
            ..close_query
        };
        assert_eq!(early_close.check_admissible(&stamp), Ok(false));
    }

    /// BD-309: fire-assay turnarounds of 3 to 21 days (plan §6.3). Every routing decision taken while the
    /// certificate is outstanding, to the last nanosecond, refuses with FMINE-REF-108 (G-TIME-2). From the instant it
    /// is available the routing query admits it, and a period close admits it at any decision time.
    #[test]
    fn test_fire_assay_lag_of_3_to_21_days_is_refused_until_available() {
        let collected = 2 * NS_PER_DAY + 7 * NS_PER_HOUR;
        let period = ValidTime::new(0, 30 * NS_PER_DAY).unwrap();
        let query = |t: i64, purpose| AsOfQuery {
            as_of_decision_ns: t,
            anchor: site_at(1),
            for_physical_period: period,
            purpose,
        };
        for lag_days in 3..=21 {
            let available = collected + lag_days * NS_PER_DAY;
            let stamp = QuadTemporalStamp::new(
                ValidTime::instant(collected),
                collected,
                available,
                RecordTime::new(SITE, 1, collected + NS_PER_HOUR, TimeEvidence::HostSynced),
            )
            .unwrap();
            let outstanding = (collected..available)
                .step_by(6 * NS_PER_HOUR as usize)
                .chain([available - 1]);
            for t in outstanding {
                let err = query(t, QueryPurpose::OperationalDecision)
                    .check_admissible(&stamp)
                    .expect_err("certificate outstanding");
                assert_eq!(
                    err,
                    TimeError::AssayLagLookahead {
                        availability_time: available,
                        as_of_decision_ns: t,
                    },
                    "lag {lag_days} d, decision at {t}"
                );
                assert_eq!(err.refusal_code(), Some("FMINE-REF-108"));
                assert_eq!(
                    query(t, QueryPurpose::RetrospectiveAccounting).check_admissible(&stamp),
                    Ok(true)
                );
            }
            for t in [available, available + NS_PER_DAY] {
                assert_eq!(
                    query(t, QueryPurpose::OperationalDecision).check_admissible(&stamp),
                    Ok(true),
                    "lag {lag_days} d, decision at {t}"
                );
            }
        }
    }

    const SITE: &str = "CELL-SITE(lumwana)";

    fn site_at(seq: u64) -> Anchor {
        Anchor::new([(SITE, seq)]).unwrap()
    }

    /// A stamp committed as `SITE` commit 1, with the commit's wall-clock reading at `record`.
    fn stamp_at(
        collection: i64,
        availability: i64,
        record: i64,
    ) -> Result<QuadTemporalStamp, TimeError> {
        QuadTemporalStamp::new(
            ValidTime::new(0, NS_PER_HOUR).unwrap(),
            collection,
            availability,
            RecordTime::new(SITE, 1, record, TimeEvidence::HostSynced),
        )
    }

    #[test]
    fn test_stamp_keeps_the_four_times_apart() {
        let stamp = stamp_at(1, 2, 3).unwrap();
        assert_eq!(
            stamp.physical_valid_time(),
            ValidTime::new(0, NS_PER_HOUR).unwrap()
        );
        assert_eq!(stamp.collection_time(), 1);
        assert_eq!(stamp.availability_time(), 2);
        let record = stamp.record_time();
        assert_eq!(
            (record.cell(), record.seq(), record.wall_ns()),
            (SITE, 1, 3)
        );
        assert_eq!(record.evidence(), TimeEvidence::HostSynced);
    }

    #[test]
    fn test_stamp_refuses_availability_before_collection() {
        assert_eq!(
            stamp_at(10, 9, 10),
            Err(TimeError::StampOrder {
                field: "availability_time",
                at_ns: 9,
                collection_time: 10,
            })
        );
    }

    #[test]
    fn test_stamp_refuses_record_before_collection() {
        assert_eq!(
            stamp_at(10, 10, 9),
            Err(TimeError::StampOrder {
                field: "record_time",
                at_ns: 9,
                collection_time: 10,
            })
        );
    }

    /// Ownership, tenure and fiscal records have no sampling lag (§6.3), and a ledger may commit a sample before
    /// its certificate arrives: neither is an impossible order.
    #[test]
    fn test_stamp_admits_no_lag_and_a_record_before_availability() {
        assert!(stamp_at(10, 10, 10).is_ok());
        assert!(stamp_at(10, 30, 20).is_ok());
    }

    /// A reading collected on day 1 and available on day 4 is knowable from day 4 at an anchor that holds its commit,
    /// and at no instant at an anchor that does not.
    #[test]
    fn test_late_reading_is_invisible_until_available_and_committed() {
        let day = 24 * NS_PER_HOUR;
        let stamp = stamp_at(day, 4 * day, 5 * day).unwrap();
        for t in [day, 4 * day, 5 * day, 9 * day] {
            assert_eq!(stamp.as_of(t, &site_at(0)), Ok(false), "not yet committed");
        }
        let held = site_at(1);
        assert_eq!(stamp.as_of(day, &held), Ok(false));
        assert_eq!(stamp.as_of(4 * day - 1, &held), Ok(false));
        assert_eq!(stamp.as_of(4 * day, &held), Ok(true));
        assert_eq!(stamp.as_of(9 * day, &held), Ok(true));
    }

    /// BD-028: commits of one cell are ordered by `seq`, even when the wall clocks run the other way; records of two
    /// cells are not ordered at all.
    #[test]
    fn test_record_order_is_by_seq_within_a_cell_and_none_across_cells() {
        let first = RecordTime::new("A", 1, 5_000, TimeEvidence::OfflineUncertain);
        let second = RecordTime::new("A", 2, 10, TimeEvidence::HostSynced);
        assert_eq!(first.commit_order(&second), Some(Ordering::Less));
        assert_eq!(second.commit_order(&first), Some(Ordering::Greater));
        assert_eq!(first.commit_order(&first.clone()), Some(Ordering::Equal));
        let other = RecordTime::new("B", 1, 5_000, TimeEvidence::HostSynced);
        assert_eq!(
            first.commit_order(&other),
            None,
            "same seq, same wall clock"
        );
        assert_eq!(second.commit_order(&other), None);
    }

    /// BD-028, INV-012: which records of two cells a consumer sees is set by the anchor's marks, never by the
    /// commits' wall clocks. Seeded: moving every wall clock leaves every answer unchanged, and an anchor that is at
    /// least as far along in every cell sees everything a lesser one sees.
    #[test]
    fn test_cross_cell_visibility_follows_the_anchor_not_the_wall_clock() {
        // The pit commit is stamped long after the mill commit, yet only the pit commit is in this anchor.
        let pit = RecordTime::new("PIT", 3, 9_000, TimeEvidence::HostSynced);
        let mill = RecordTime::new("MILL", 1, 10, TimeEvidence::HostSynced);
        let anchor = Anchor::new([("PIT", 3), ("MILL", 0)]).unwrap();
        assert_eq!(anchor.has_committed(&pit), Ok(true));
        assert_eq!(anchor.has_committed(&mill), Ok(false));

        const SEED: u64 = 0x0028_A4C0;
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: SEED,
            partition: 0,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let cells = ["PIT", "MILL", "LAB"];
        for _ in 0..10_000 {
            let cell = cells[(rng.next_u64() % 3) as usize];
            let seq = rng.next_u64() % 50;
            let lesser: Vec<(&str, u64)> =
                cells.iter().map(|&c| (c, rng.next_u64() % 50)).collect();
            let greater: Vec<(&str, u64)> = lesser
                .iter()
                .map(|&(c, m)| (c, m + rng.next_u64() % 5))
                .collect();
            let (lesser, greater) = (Anchor::new(lesser).unwrap(), Anchor::new(greater).unwrap());
            let wall = (rng.next_u64() % 1_000_000) as i64;
            let record = RecordTime::new(cell, seq, wall, TimeEvidence::HostSynced);
            let moved = RecordTime::new(cell, seq, -wall, TimeEvidence::OfflineUncertain);
            let seen = lesser.has_committed(&record).unwrap();
            assert_eq!(seen, seq <= lesser.mark(cell).unwrap(), "seed {SEED:#x}");
            assert_eq!(lesser.has_committed(&moved), Ok(seen), "seed {SEED:#x}");
            assert!(
                !seen || greater.has_committed(&record).unwrap(),
                "seed {SEED:#x}: {record:?} seen at {lesser:?}, not at {greater:?}"
            );
        }
    }

    /// INV-012: a record whose cell the anchor does not name is refused as not observable. It is not quietly left
    /// out, as a record the anchor does not yet hold is.
    #[test]
    fn test_cell_missing_from_the_anchor_is_not_observable() {
        let stamp = stamp_at(1, 2, 3).unwrap();
        let elsewhere = Anchor::new([("CELL-PUBLIC", 99)]).unwrap();
        let not_observable = TimeError::CellNotInAnchor {
            cell: SITE.to_string(),
            seq: 1,
        };
        let refused = Err(not_observable.clone());
        assert_eq!(stamp.as_of(10, &elsewhere), refused);
        assert_eq!(stamp.as_of(10, &Anchor::default()), refused);
        for purpose in [
            QueryPurpose::OperationalDecision,
            QueryPurpose::RetrospectiveAccounting,
        ] {
            let query = AsOfQuery {
                as_of_decision_ns: 10,
                anchor: elsewhere.clone(),
                for_physical_period: ValidTime::new(0, NS_PER_HOUR).unwrap(),
                purpose,
            };
            assert_eq!(query.check_admissible(&stamp), refused);
        }
        let message = not_observable.to_string();
        assert!(message.contains(SITE) && message.contains("not observable"));
    }

    /// An anchor holds one mark per cell, read from the cell's `LedgerAnchor`; a cell named twice is refused.
    #[test]
    fn test_anchor_reads_ledger_anchors_and_refuses_a_cell_named_twice() {
        let ledger = |cell: &str, commit_sequence: u64| LedgerAnchor {
            format_version: fmine_core::FMT_ANCHOR_V1,
            cell_id: cell.to_string(),
            ledger_epoch: 4,
            commit_sequence,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: fmine_core::Digest32([7; 32]),
        };
        let anchor =
            Anchor::from_ledger_anchors(&[ledger("CELL-PUBLIC", 12), ledger(SITE, 40)]).unwrap();
        assert_eq!(anchor.mark("CELL-PUBLIC"), Some(12));
        assert_eq!(anchor.mark(SITE), Some(40));
        assert_eq!(anchor.mark("CELL-DEC"), None);

        let twice = Err(TimeError::AnchorCellRepeated {
            cell: SITE.to_string(),
        });
        assert_eq!(
            Anchor::from_ledger_anchors(&[ledger(SITE, 40), ledger(SITE, 41)]),
            twice
        );
        assert_eq!(Anchor::new([(SITE, 5), ("A", 1), (SITE, 5)]), twice);
    }

    /// Once a record is knowable it stays knowable, and the first such instant is exact.
    #[test]
    fn test_as_of_is_monotone_in_t() {
        const SEED: u64 = 0x0309_F00D;
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: SEED,
            partition: 0,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut draw = |span: u64| (rng.next_u64() % span) as i64;
        for _ in 0..10_000 {
            let collection = draw(1_000);
            let (availability, record) = (collection + draw(1_000), collection + draw(1_000));
            let stamp = stamp_at(collection, availability, record).unwrap();
            let (a, b) = (draw(3_000), draw(3_000));
            let (t0, t1) = (a.min(b), a.max(b));
            let held = site_at(1);
            let knowable = |t: i64| stamp.as_of(t, &held).unwrap();
            assert!(
                !knowable(t0) || knowable(t1),
                "seed {SEED:#x}: {stamp:?} knowable at {t0}, not at {t1}"
            );
            assert!(
                knowable(availability) && !knowable(availability - 1),
                "seed {SEED:#x}: {stamp:?} first knowable at {availability}"
            );
        }
    }
}
