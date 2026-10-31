//! Quad-temporal timestamps (`physical_valid_time`, `collection_time`, `availability_time`, `record_time`; plan §6.3),
//! civil calendar arithmetic, site/fiscal calendars (`G-TIME-1`), and assay-lag lookahead
//! guards (`G-TIME-2`, `FMINE-REF-108`).

use std::fmt;

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

/// Ledger ingestion / transaction timestamp (`UTC` nanoseconds since Unix epoch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordTime {
    pub utc_ns: i64,
    pub evidence: TimeEvidence,
}

impl RecordTime {
    /// Creates a `RecordTime` at `utc_ns`.
    #[must_use]
    pub const fn new(utc_ns: i64, evidence: TimeEvidence) -> Self {
        Self { utc_ns, evidence }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadTemporalStamp {
    physical_valid_time: ValidTime,
    collection_time: i64,
    availability_time: i64,
    record_time: RecordTime,
}

impl QuadTemporalStamp {
    /// Builds a stamp. Refuses `availability_time` or `record_time` before `collection_time`
    /// ([`TimeError::StampOrder`]); nothing is reordered or clamped.
    pub const fn new(
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
        if record_time.utc_ns < collection_time {
            return Err(TimeError::StampOrder {
                field: "record_time",
                at_ns: record_time.utc_ns,
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

    /// When the ledger committed it.
    #[must_use]
    pub const fn record_time(&self) -> RecordTime {
        self.record_time
    }

    /// Whether the record was knowable at `t_ns`: `availability_time ≤ t` and `record_time ≤ t` (§6.3, `INV-016`).
    #[must_use]
    pub const fn as_of(&self, t_ns: i64) -> bool {
        self.availability_time <= t_ns && self.record_time.utc_ns <= t_ns
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
    /// `record_time.utc_ns <= as_of_decision_ns`.
    RetrospectiveAccounting,
}

/// Dual-clock `AS OF` query (`BD-032`, `BD-309`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsOfQuery {
    /// Knowledge cut-off timestamp (`as_of_decision`).
    pub as_of_decision_ns: i64,
    /// Physical production window being queried (`for_physical_period`).
    pub for_physical_period: ValidTime,
    /// Whether this query drives a real-time routing decision or a retrospective period close.
    pub purpose: QueryPurpose,
}

impl AsOfQuery {
    /// Evaluates whether a record with `stamp` is admissible under this `AsOfQuery`.
    ///
    /// - If `stamp.physical_valid_time()` does not overlap `self.for_physical_period`, returns `Ok(false)`.
    /// - If `stamp.record_time().utc_ns > self.as_of_decision_ns`, returns `Ok(false)`.
    /// - Under `QueryPurpose::OperationalDecision`, if `stamp.availability_time() > self.as_of_decision_ns`,
    ///   returns `Err(TimeError::AssayLagLookahead)` (`FMINE-REF-108`, `G-TIME-2`).
    pub fn check_admissible(&self, stamp: &QuadTemporalStamp) -> Result<bool, TimeError> {
        if !stamp.physical_valid_time.overlaps(self.for_physical_period) {
            return Ok(false);
        }
        if stamp.record_time.utc_ns > self.as_of_decision_ns {
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
            RecordTime::new(t_blast + NS_PER_HOUR, TimeEvidence::HostSynced),
        )
        .unwrap();

        // 1. Real-time routing query at t_route = 12h MUST refuse the t = 36h assay (FMINE-REF-108).
        let routing_query = AsOfQuery {
            as_of_decision_ns: t_route,
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
            for_physical_period: ValidTime::new(0, 24 * NS_PER_HOUR).unwrap(),
            purpose: QueryPurpose::RetrospectiveAccounting,
        };
        assert!(
            close_query
                .check_admissible(&stamp)
                .expect("admissible at close")
        );
    }

    fn stamp_at(
        collection: i64,
        availability: i64,
        record: i64,
    ) -> Result<QuadTemporalStamp, TimeError> {
        QuadTemporalStamp::new(
            ValidTime::new(0, NS_PER_HOUR).unwrap(),
            collection,
            availability,
            RecordTime::new(record, TimeEvidence::HostSynced),
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
        assert_eq!(stamp.record_time().utc_ns, 3);
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

    /// A reading collected on day 1, available on day 4 and recorded on day 5 is knowable from day 5, and from that
    /// instant on. No other instant in between shows it.
    #[test]
    fn test_late_reading_is_invisible_until_available_and_recorded() {
        let day = 24 * NS_PER_HOUR;
        let stamp = stamp_at(day, 4 * day, 5 * day).unwrap();
        assert!(!stamp.as_of(day));
        assert!(!stamp.as_of(4 * day), "available but not yet recorded");
        assert!(!stamp.as_of(5 * day - 1));
        assert!(stamp.as_of(5 * day));
        assert!(stamp.as_of(6 * day));
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
            assert!(
                !stamp.as_of(t0) || stamp.as_of(t1),
                "seed {SEED:#x}: {stamp:?} knowable at {t0}, not at {t1}"
            );
            let first = availability.max(record);
            assert!(
                stamp.as_of(first) && !stamp.as_of(first - 1),
                "seed {SEED:#x}: {stamp:?} first knowable at {first}"
            );
        }
    }
}
