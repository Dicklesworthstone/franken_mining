//! The public forecast record (BD-334, plan §21.6): a manifest hashed together with the forecasts, so that they
//! can be scored later without judgment, and a rules file hashed and stamped before any forecast exists.
//! Determinism: Exact (integers, text and content hashes; no floating point outside tests).
//!
//! Everything here is pure: the two frames, the manifest's parser and validator, the exact interval score and
//! the persistence baseline's quantiles. Reading the files, writing the digests and reading a time proof's dates
//! are `xtask forecast-*`, because `fmine-calib` (K0) does no IO. A time is only ever taken from a proof
//! ([`ProofTimes`]), never from the caller's clock.
//!
//! The manifest is written in `fmine-registry`'s strict TOML subset: top-level keys, then one `[[input]]` row
//! per document. The subset has no booleans, so the two flags are the plan's own words:
//! - `label = "forecast" | "nowcast"`: a record time-proved at or after the issuer's Q3 release is a nowcast and
//!   is scored separately (§21.6, R7-C119);
//! - `contact = "pre-contact" | "post-contact"`: any revision after first issuer contact is flagged.

use std::cmp::Ordering;
use std::fmt;
use std::ops::Range;

use fmine_core::hash::{Digest32, sha256};
pub use fmine_numerics::exact_feas::Rat;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_time::CivilDate;

/// The record frame's tag: `FMREC1 LF ‖ dec(len manifest) LF ‖ manifest ‖ dec(len csv) LF ‖ csv`.
pub const RECORD_TAG: &[u8] = b"FMREC1\n";
/// The rules frame's tag: `FMRUL1 LF ‖ dec(len rules) LF ‖ rules`.
pub const RULES_TAG: &[u8] = b"FMRUL1\n";
/// The attestation every record carries, word for word (§21.6).
pub const ATTESTATION: &str = "No non-public issuer information was used to make these forecasts.";

const SECONDS_PER_DAY: i64 = 86_400;

/// The dates a record is judged against, in UTC seconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordDates {
    /// A record whose proof's upper time is at or after this is a nowcast.
    pub nowcast_from: i64,
    /// The proof's upper time must not be later than this.
    pub proof_by: i64,
}

const fn utc(year: i32, month: u8, day: u8, hour: i64, minute: i64) -> i64 {
    CivilDate { year, month, day }.to_epoch_days() * SECONDS_PER_DAY + hour * 3_600 + minute * 60
}

/// FY2026 (§21.6): Barrick releases Q3 2026 at 06:00 ET on 9 Nov 2026, which is 11:00 UTC because US daylight
/// saving time ends on 1 Nov (E454); the time proof is due by the end of 31 Dec 2026.
pub const FY2026_DATES: RecordDates = RecordDates {
    nowcast_from: utc(2026, 11, 9, 11, 0),
    proof_by: utc(2027, 1, 1, 0, 0) - 1,
};

fn push_part(out: &mut Vec<u8>, part: &[u8]) {
    out.extend_from_slice(part.len().to_string().as_bytes());
    out.push(b'\n');
    out.extend_from_slice(part);
}

/// The manifest and the forecasts as one frame, so that a change to either byte stream moves the digest and no
/// byte can migrate from one file to the other.
#[must_use]
pub fn record_frame(manifest: &[u8], forecasts_csv: &[u8]) -> Vec<u8> {
    let mut out = RECORD_TAG.to_vec();
    push_part(&mut out, manifest);
    push_part(&mut out, forecasts_csv);
    out
}

/// The rules file as a frame of its own, hashed and stamped before any forecast exists (BD-334.S2).
#[must_use]
pub fn rules_frame(rules: &[u8]) -> Vec<u8> {
    let mut out = RULES_TAG.to_vec();
    push_part(&mut out, rules);
    out
}

/// SHA-256 of [`record_frame`].
#[must_use]
pub fn record_digest(manifest: &[u8], forecasts_csv: &[u8]) -> Digest32 {
    sha256(&record_frame(manifest, forecasts_csv))
}

/// SHA-256 of [`rules_frame`].
#[must_use]
pub fn rules_digest(rules: &[u8]) -> Digest32 {
    sha256(&rules_frame(rules))
}

/// Parses 64 lower-case hex digits; anything else is `None`.
#[must_use]
pub fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0_u8; 32];
    for (o, [hi, lo]) in out.iter_mut().zip(b.as_chunks::<2>().0) {
        *o = (nibble(*hi)? << 4) | nibble(*lo)?;
    }
    Some(out)
}

fn digits(b: &[u8], r: Range<usize>) -> Option<u32> {
    b[r].iter().try_fold(0_u32, |n, c| {
        c.is_ascii_digit().then(|| n * 10 + u32::from(c - b'0'))
    })
}

fn epoch_day(b: &[u8]) -> Option<i64> {
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let date = CivilDate::new(
        i32::try_from(digits(b, 0..4)?).ok()?,
        u8::try_from(digits(b, 5..7)?).ok()?,
        u8::try_from(digits(b, 8..10)?).ok()?,
    )
    .ok()?;
    Some(date.to_epoch_days())
}

/// Parses a calendar date `YYYY-MM-DD` to days since the Unix epoch.
#[must_use]
pub fn parse_date(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 {
        return None;
    }
    epoch_day(b)
}

/// Parses `YYYY-MM-DDTHH:MM:SSZ` to seconds since the Unix epoch. Offsets other than `Z`, fractions and leap
/// seconds are refused.
#[must_use]
pub fn parse_utc(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let day = epoch_day(&b[..10])?;
    let (h, m, sec) = (digits(b, 11..13)?, digits(b, 14..16)?, digits(b, 17..19)?);
    if h > 23 || m > 59 || sec > 59 {
        return None;
    }
    Some(day * SECONDS_PER_DAY + i64::from(h * 3_600 + m * 60 + sec))
}

/// The inverse of [`parse_utc`].
#[must_use]
pub fn format_utc(t: i64) -> String {
    let d = CivilDate::from_epoch_days(t.div_euclid(SECONDS_PER_DAY));
    let s = t.rem_euclid(SECONDS_PER_DAY);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        d.year,
        d.month,
        d.day,
        s / 3_600,
        s % 3_600 / 60,
        s % 60
    )
}

/// Whether the record was time-proved before the issuer's Q3 release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Forecast,
    Nowcast,
}

impl Label {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forecast => "forecast",
            Self::Nowcast => "nowcast",
        }
    }
}

/// Whether the record was made, or last revised, after first contact with the issuer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    PreContact,
    PostContact,
}

/// One document in the information set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDoc {
    /// The file name, as the source registry spells it for a registered file.
    pub doc_id: String,
    pub url: String,
    /// The issuer's publication date, in days since the Unix epoch.
    pub publication_day: i64,
    pub retrieved_utc: i64,
    pub bytes: u64,
    pub sha256: [u8; 32],
    /// The 1-based line of the row's `[[input]]` header.
    pub line: usize,
}

/// A parsed `manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordManifest {
    pub record_id: String,
    /// A role id such as `public-lane-author`; never a personal name.
    pub author_role: String,
    pub information_cutoff_utc: i64,
    pub label: Label,
    pub contact: Contact,
    pub attestation: String,
    /// The FMRUL1 digest of the rules file stamped before the forecasts (BD-334.S2).
    pub rules_sha256: [u8; 32],
    pub inputs: Vec<InputDoc>,
}

/// Why a manifest, a score or a quantile was refused. Nothing is defaulted, clamped or skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordError {
    /// A line outside the TOML subset.
    Syntax {
        line: usize,
        reason: String,
    },
    UnknownTable {
        line: usize,
        table: String,
    },
    UnknownKey {
        line: usize,
        key: String,
    },
    /// A required key is absent from the top level (`line` 0) or from the row starting on `line`.
    Missing {
        line: usize,
        key: &'static str,
    },
    BadValue {
        line: usize,
        key: &'static str,
        expected: &'static str,
    },
    NoInputs,
    /// The interval level α is outside (0, 1).
    AlphaOutOfDomain,
    /// The interval's lower bound exceeds its upper bound.
    InvertedInterval,
    EmptySample,
    /// A quantile level outside [0, 1].
    LevelOutOfDomain,
    /// A ratio `Q_t+1 / Q_t` that is not positive has no logarithm.
    NonPositiveRatio,
    /// The linear estimator interpolates log ratios, which makes the bounds irrational; the exact record uses
    /// the inverse empirical CDF instead of an approximation.
    NotExact,
    /// An exact operation left the `i128` range.
    Overflow,
    /// The rules still carry an `OPEN` marker: an undecided point cannot be stamped.
    OpenMarker {
        line: usize,
    },
    /// A second row with the same id (a site name, or a baseline id for the same target).
    DuplicateId {
        line: usize,
        id: String,
    },
    /// A baseline names a target the rules do not declare.
    UnknownTarget {
        line: usize,
        target: String,
    },
    /// A target with no baseline cannot be scored against one.
    TargetWithoutBaseline {
        target: String,
    },
    /// The rules declare no row of a table they need.
    NoRows {
        table: &'static str,
    },
    /// A baseline id the scoring code has no rule for.
    UnknownBaseline {
        line: usize,
        id: String,
    },
    /// A line of `forecasts.csv` that is not a forecast of the rules' universe (1-based; line 1 is the header).
    Forecast {
        line: usize,
        reason: String,
    },
    /// A live site × target of the rules with no forecast row.
    MissingForecast {
        site: String,
        target: String,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { line, reason } => write!(f, "line {line}: {reason}"),
            Self::UnknownTable { line, table } => {
                write!(f, "line {line}: unknown table [[{table}]]")
            }
            Self::UnknownKey { line, key } => write!(f, "line {line}: unknown key {key:?}"),
            Self::Missing { line: 0, key } => write!(f, "no top-level {key}"),
            Self::Missing { line, key } => {
                write!(f, "the row starting on line {line} has no {key}")
            }
            Self::BadValue {
                line,
                key,
                expected,
            } => write!(f, "line {line}: {key} must be {expected}"),
            Self::NoInputs => write!(f, "the manifest lists no [[input]] document"),
            Self::AlphaOutOfDomain => write!(f, "the interval level alpha must lie in (0, 1)"),
            Self::InvertedInterval => {
                write!(f, "the interval's lower bound exceeds its upper bound")
            }
            Self::EmptySample => write!(f, "a quantile needs at least one value"),
            Self::LevelOutOfDomain => write!(f, "a quantile level must lie in [0, 1]"),
            Self::NonPositiveRatio => {
                write!(f, "a run-rate ratio must be positive to have a logarithm")
            }
            Self::NotExact => write!(
                f,
                "the linear estimator interpolates log ratios, so its bounds are irrational; use the inverse \
                 empirical CDF"
            ),
            Self::Overflow => write!(f, "an exact operation left the i128 range"),
            Self::OpenMarker { line } => {
                write!(
                    f,
                    "line {line}: an OPEN point must be decided before the rules are stamped"
                )
            }
            Self::DuplicateId { line, id } => write!(f, "line {line}: {id:?} is listed twice"),
            Self::UnknownTarget { line, target } => {
                write!(
                    f,
                    "line {line}: baseline target {target:?} is not a declared [[target]]"
                )
            }
            Self::TargetWithoutBaseline { target } => {
                write!(f, "target {target:?} has no [[baseline]]")
            }
            Self::NoRows { table } => write!(f, "no [[{table}]] row"),
            Self::UnknownBaseline { line, id } => write!(
                f,
                "line {line}: baseline {id:?} is neither issuer_guidance nor run_rate_persistence"
            ),
            Self::Forecast { line, reason } => write!(f, "forecasts.csv line {line}: {reason}"),
            Self::MissingForecast { site, target } => {
                write!(f, "forecasts.csv has no {target} forecast for {site}")
            }
        }
    }
}

impl std::error::Error for RecordError {}

const TOP_KEYS: [&str; 7] = [
    "record_id",
    "author_role",
    "information_cutoff_utc",
    "label",
    "contact",
    "attestation",
    "rules_sha256",
];
const INPUT_KEYS: [&str; 6] = [
    "doc_id",
    "url",
    "publication_date",
    "retrieved_utc",
    "bytes",
    "sha256",
];

/// Typed reads over one block of fields: the top level (`line` 0) or one `[[input]]` row.
struct Fields<'a> {
    fields: &'a [TomlField],
    line: usize,
}

impl Fields<'_> {
    fn check_keys(&self, allowed: &[&str]) -> Result<(), RecordError> {
        match self
            .fields
            .iter()
            .find(|f| !allowed.contains(&f.key.as_str()))
        {
            Some(f) => Err(RecordError::UnknownKey {
                line: f.line,
                key: f.key.clone(),
            }),
            None => Ok(()),
        }
    }

    fn field(&self, key: &'static str) -> Result<&TomlField, RecordError> {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .ok_or(RecordError::Missing {
                line: self.line,
                key,
            })
    }

    fn text(&self, key: &'static str) -> Result<(&str, usize), RecordError> {
        let f = self.field(key)?;
        match &f.value {
            TomlValue::Str(s) if !s.trim().is_empty() => Ok((s.as_str(), f.line)),
            _ => Err(RecordError::BadValue {
                line: f.line,
                key,
                expected: "a non-empty string",
            }),
        }
    }

    fn parsed<T>(
        &self,
        key: &'static str,
        expected: &'static str,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Result<T, RecordError> {
        let (s, line) = self.text(key)?;
        parse(s).ok_or(RecordError::BadValue {
            line,
            key,
            expected,
        })
    }

    fn positive(&self, key: &'static str) -> Result<u64, RecordError> {
        let f = self.field(key)?;
        match f.value {
            TomlValue::Int(n) if n > 0 => Ok(n.unsigned_abs()),
            _ => Err(RecordError::BadValue {
                line: f.line,
                key,
                expected: "a positive integer",
            }),
        }
    }
}

/// Parses `manifest.toml`: every key required, no other key or table, each value of its type.
pub fn parse_manifest(text: &str) -> Result<RecordManifest, RecordError> {
    let doc = parse_subset(text).map_err(|e| RecordError::Syntax {
        line: e.line,
        reason: e.reason,
    })?;
    let top = Fields {
        fields: &doc.top,
        line: 0,
    };
    top.check_keys(&TOP_KEYS)?;
    let mut inputs = Vec::new();
    for row in &doc.rows {
        if row.table != "input" {
            return Err(RecordError::UnknownTable {
                line: row.line,
                table: row.table.clone(),
            });
        }
        let r = Fields {
            fields: &row.fields,
            line: row.line,
        };
        r.check_keys(&INPUT_KEYS)?;
        inputs.push(InputDoc {
            doc_id: r.text("doc_id")?.0.to_string(),
            url: r.text("url")?.0.to_string(),
            publication_day: r.parsed("publication_date", "a date YYYY-MM-DD", parse_date)?,
            retrieved_utc: r.parsed("retrieved_utc", "a time YYYY-MM-DDTHH:MM:SSZ", parse_utc)?,
            bytes: r.positive("bytes")?,
            sha256: r.parsed("sha256", "64 lower-case hex digits", parse_hex32)?,
            line: row.line,
        });
    }
    let manifest = RecordManifest {
        record_id: top.text("record_id")?.0.to_string(),
        author_role: top.text("author_role")?.0.to_string(),
        information_cutoff_utc: top.parsed(
            "information_cutoff_utc",
            "a time YYYY-MM-DDTHH:MM:SSZ",
            parse_utc,
        )?,
        label: top.parsed("label", "\"forecast\" or \"nowcast\"", |s| match s {
            "forecast" => Some(Label::Forecast),
            "nowcast" => Some(Label::Nowcast),
            _ => None,
        })?,
        contact: top.parsed(
            "contact",
            "\"pre-contact\" or \"post-contact\"",
            |s| match s {
                "pre-contact" => Some(Contact::PreContact),
                "post-contact" => Some(Contact::PostContact),
                _ => None,
            },
        )?,
        attestation: top.text("attestation")?.0.to_string(),
        rules_sha256: top.parsed("rules_sha256", "64 lower-case hex digits", parse_hex32)?,
        inputs,
    };
    if manifest.inputs.is_empty() {
        return Err(RecordError::NoInputs);
    }
    Ok(manifest)
}

/// A reporting quarter, `YYYYQn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quarter {
    pub year: i32,
    /// 1–4.
    pub q: u8,
}

impl Quarter {
    /// `2026Q3`; anything else is `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (y, q) = s.split_once('Q')?;
        if y.len() != 4 || !y.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let q = match q {
            "1" => 1,
            "2" => 2,
            "3" => 3,
            "4" => 4,
            _ => return None,
        };
        Some(Self {
            year: y.parse().ok()?,
            q,
        })
    }

    #[must_use]
    pub const fn next(self) -> Self {
        if self.q == 4 {
            Self {
                year: self.year + 1,
                q: 1,
            }
        } else {
            Self {
                year: self.year,
                q: self.q + 1,
            }
        }
    }

    /// The quarters from `self` to `last`, both included; empty when `last` is earlier.
    #[must_use]
    pub fn through(self, last: Self) -> Vec<Self> {
        let mut out = Vec::new();
        let mut q = self;
        while q <= last {
            out.push(q);
            q = q.next();
        }
        out
    }
}

impl fmt::Display for Quarter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Q{}", self.year, self.q)
    }
}

/// How a target's outcome is read from the outcome release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// `fy_minus_9m`: the full year (YTD sheet, Q4 release) minus the nine months (YTD sheet, Q3 release); an
    /// additive quantity only.
    FyMinus9m,
    /// `q4_qtd`: the Q4 value as printed (QTD sheet, Q4 release); a per-unit rate, which is not additive.
    Q4Qtd,
}

/// A forecast target: what is forecast and what is scored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub quantity: String,
    pub scored_quantity: String,
    /// The Mine Stats summary field id the target is read from (e.g. `au_prod_100_koz`).
    pub field: String,
    pub outcome: Outcome,
}

/// What a baseline row computes. The id decides it, so the scoring code never reads the prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineKind {
    /// `issuer_guidance`: the issuer's own range, transcribed at scoring by two readers (BD-341).
    IssuerGuidance,
    /// `run_rate_persistence`: `point` quarter's value × the quantiles of the ratios of consecutive quarters
    /// `pool_from..=pool_through`.
    Persistence {
        pool_from: Quarter,
        pool_through: Quarter,
        point: Quarter,
    },
}

/// A pre-written rule for a change of basis (a divestment, a re-segmentation, an ownership change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasisRule {
    pub id: String,
    pub rule: String,
}

/// One baseline for one target, given in words and resolved at scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRule {
    pub id: String,
    pub kind: BaselineKind,
    pub target: String,
    pub rule: String,
    pub source: String,
    /// The 1-based line of the row's `[[baseline]]` header.
    pub line: usize,
}

/// A site's place in the record's universe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteStatus {
    Live,
    /// Listed and counted, never dropped: its releases print no value (a divestment, say).
    NotPublished,
}

/// One site of the universe, named as the label its locator points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesSite {
    pub name: String,
    pub status: SiteStatus,
    pub locator: String,
    /// Why a `NotPublished` site prints nothing; `None` for a live site.
    pub reason: Option<String>,
}

/// A parsed `rules.toml`: the typed parts code scores with. The prose (the interval score's formula, the report,
/// the calibration claim, the outcome vintage, the excluded rows) must be present but is bound only by the
/// rules' digest.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordRules {
    pub rules_id: String,
    /// The record these rules govern; its manifest carries the same id.
    pub record_id: String,
    /// The source registry's issuer id.
    pub issuer: String,
    pub alpha: Rat,
    pub estimator: Estimator,
    pub rho_low: Rat,
    pub rho_high: Rat,
    pub coverage_level: Rat,
    pub calibration_claim_min_n: u64,
    pub dates: RecordDates,
    pub targets: Vec<Target>,
    pub basis_rules: Vec<BasisRule>,
    pub baselines: Vec<BaselineRule>,
    pub sites: Vec<RulesSite>,
}

const RULES_PROSE_KEYS: [&str; 5] = [
    "interval_score",
    "report",
    "calibration_claim",
    "outcome_vintage",
    "excluded_rows",
];
const RULES_TOP_KEYS: [&str; 16] = [
    "rules_id",
    "record_id",
    "issuer",
    "alpha",
    "quantile_estimator",
    "rho_low",
    "rho_high",
    "coverage_level",
    "calibration_claim_min_n",
    "nowcast_from_utc",
    "proof_by_utc",
    "interval_score",
    "report",
    "calibration_claim",
    "outcome_vintage",
    "excluded_rows",
];

/// Whether `s` holds the word `OPEN`, the marker of a point still to be decided.
#[must_use]
pub fn has_open_marker(s: &str) -> bool {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "OPEN")
}

/// A decimal in (0, 1), or in [0, 1) when `zero_ok`.
fn unit_decimal(s: &str, zero_ok: bool) -> Option<Rat> {
    let x = Rat::from_decimal_str(s).ok()?;
    let below_one = x.checked_cmp(Rat::from_integer(1)).ok()? == Ordering::Less;
    let lower = if zero_ok {
        x.signum() >= 0
    } else {
        x.signum() > 0
    };
    (below_one && lower).then_some(x)
}

/// Parses `rules.toml` (BD-334.S2): every key required, no other key or table, each value of its type, no `OPEN`
/// marker in any value, unique ids, every baseline on a declared target and every target with a baseline.
pub fn parse_rules(text: &str) -> Result<RecordRules, RecordError> {
    let doc = parse_subset(text).map_err(|e| RecordError::Syntax {
        line: e.line,
        reason: e.reason,
    })?;
    let mut values = doc
        .top
        .iter()
        .chain(doc.rows.iter().flat_map(|r| r.fields.iter()));
    if let Some(f) = values.find(|f| matches!(&f.value, TomlValue::Str(s) if has_open_marker(s))) {
        return Err(RecordError::OpenMarker { line: f.line });
    }
    let top = Fields {
        fields: &doc.top,
        line: 0,
    };
    top.check_keys(&RULES_TOP_KEYS)?;
    for key in RULES_PROSE_KEYS {
        top.text(key)?;
    }
    let (mut targets, mut basis_rules, mut baselines, mut sites) = (
        Vec::<Target>::new(),
        Vec::<BasisRule>::new(),
        Vec::<BaselineRule>::new(),
        Vec::<RulesSite>::new(),
    );
    for row in &doc.rows {
        let r = Fields {
            fields: &row.fields,
            line: row.line,
        };
        let text = |key| r.text(key).map(|(s, _)| s.to_string());
        let duplicate = |id: &str| RecordError::DuplicateId {
            line: row.line,
            id: id.to_string(),
        };
        match row.table.as_str() {
            "target" => {
                r.check_keys(&["id", "quantity", "scored_quantity", "field", "outcome"])?;
                let id = text("id")?;
                if targets.iter().any(|t| t.id == id) {
                    return Err(duplicate(&id));
                }
                targets.push(Target {
                    id,
                    quantity: text("quantity")?,
                    scored_quantity: text("scored_quantity")?,
                    field: r.parsed("field", "a field id of [a-z0-9_]", |s| {
                        s.bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                            .then(|| s.to_string())
                    })?,
                    outcome: r.parsed("outcome", "\"fy_minus_9m\" or \"q4_qtd\"", |s| match s {
                        "fy_minus_9m" => Some(Outcome::FyMinus9m),
                        "q4_qtd" => Some(Outcome::Q4Qtd),
                        _ => None,
                    })?,
                });
            }
            "basis_rule" => {
                r.check_keys(&["id", "rule"])?;
                let id = text("id")?;
                if basis_rules.iter().any(|b| b.id == id) {
                    return Err(duplicate(&id));
                }
                basis_rules.push(BasisRule {
                    id,
                    rule: text("rule")?,
                });
            }
            "baseline" => {
                let id = text("id")?;
                let kind = match id.as_str() {
                    "issuer_guidance" => {
                        r.check_keys(&["id", "target", "rule", "source"])?;
                        BaselineKind::IssuerGuidance
                    }
                    "run_rate_persistence" => {
                        r.check_keys(&[
                            "id",
                            "target",
                            "rule",
                            "source",
                            "pool_from",
                            "pool_through",
                            "point_quarter",
                        ])?;
                        let quarter = |key| r.parsed(key, "a quarter YYYYQn", Quarter::parse);
                        let (pool_from, pool_through) =
                            (quarter("pool_from")?, quarter("pool_through")?);
                        let point = quarter("point_quarter")?;
                        // One pair at least, and the point after the pool: a point inside the pool would score
                        // the baseline on its own sample.
                        if pool_through <= pool_from || point <= pool_through {
                            return Err(RecordError::BadValue {
                                line: r.field("point_quarter")?.line,
                                key: "point_quarter",
                                expected: "after pool_through, itself after pool_from",
                            });
                        }
                        BaselineKind::Persistence {
                            pool_from,
                            pool_through,
                            point,
                        }
                    }
                    _ => {
                        return Err(RecordError::UnknownBaseline { line: row.line, id });
                    }
                };
                let target = text("target")?;
                if baselines.iter().any(|b| b.id == id && b.target == target) {
                    return Err(duplicate(&format!("{id} for {target}")));
                }
                baselines.push(BaselineRule {
                    id,
                    kind,
                    target,
                    rule: text("rule")?,
                    source: text("source")?,
                    line: row.line,
                });
            }
            "site" => {
                r.check_keys(&["name", "status", "locator", "reason"])?;
                let name = text("name")?;
                if sites.iter().any(|s| s.name == name) {
                    return Err(duplicate(&name));
                }
                let status = r.parsed("status", "\"live\" or \"not_published\"", |s| match s {
                    "live" => Some(SiteStatus::Live),
                    "not_published" => Some(SiteStatus::NotPublished),
                    _ => None,
                })?;
                let reason = match status {
                    SiteStatus::NotPublished => Some(text("reason")?),
                    SiteStatus::Live => match r.field("reason") {
                        Ok(f) => {
                            return Err(RecordError::BadValue {
                                line: f.line,
                                key: "reason",
                                expected: "absent for a live site",
                            });
                        }
                        Err(_) => None,
                    },
                };
                sites.push(RulesSite {
                    name,
                    status,
                    locator: text("locator")?,
                    reason,
                });
            }
            other => {
                return Err(RecordError::UnknownTable {
                    line: row.line,
                    table: other.to_string(),
                });
            }
        }
    }
    if let Some(b) = baselines
        .iter()
        .find(|b| !targets.iter().any(|t| t.id == b.target))
    {
        return Err(RecordError::UnknownTarget {
            line: b.line,
            target: b.target.clone(),
        });
    }
    if let Some(t) = targets
        .iter()
        .find(|t| !baselines.iter().any(|b| b.target == t.id))
    {
        return Err(RecordError::TargetWithoutBaseline {
            target: t.id.clone(),
        });
    }
    if targets.is_empty() {
        return Err(RecordError::NoRows { table: "target" });
    }
    if sites.is_empty() {
        return Err(RecordError::NoRows { table: "site" });
    }
    let rho_low = top.parsed("rho_low", "a decimal in [0, 1)", |s| unit_decimal(s, true))?;
    let rho_high = top.parsed("rho_high", "a decimal in [rho_low, 1)", |s| {
        unit_decimal(s, true).filter(|x| x.checked_cmp(rho_low).ok() != Some(Ordering::Less))
    })?;
    let utc = "a time YYYY-MM-DDTHH:MM:SSZ";
    Ok(RecordRules {
        rules_id: top.text("rules_id")?.0.to_string(),
        record_id: top.text("record_id")?.0.to_string(),
        issuer: top.text("issuer")?.0.to_string(),
        alpha: top.parsed("alpha", "a decimal in (0, 1)", |s| unit_decimal(s, false))?,
        estimator: top.parsed(
            "quantile_estimator",
            "\"inverse_ecdf\" or \"linear\"",
            |s| match s {
                "inverse_ecdf" => Some(Estimator::InverseEcdf),
                "linear" => Some(Estimator::Linear),
                _ => None,
            },
        )?,
        rho_low,
        rho_high,
        coverage_level: top.parsed("coverage_level", "a decimal in (0, 1)", |s| {
            unit_decimal(s, false)
        })?,
        calibration_claim_min_n: top.positive("calibration_claim_min_n")?,
        dates: RecordDates {
            nowcast_from: top.parsed("nowcast_from_utc", utc, parse_utc)?,
            proof_by: top.parsed("proof_by_utc", utc, parse_utc)?,
        },
        targets,
        basis_rules,
        baselines,
        sites,
    })
}

/// The interval `[lower_utc, upper_utc]` a time proof places the record's digest in: an RFC 3161 `genTime` ±
/// its stated accuracy, or an OpenTimestamps attestation's block-time bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofTimes {
    pub lower_utc: i64,
    pub upper_utc: i64,
}

/// A registered file and its digest, as the source registry holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredDigest<'a> {
    pub file: &'a str,
    pub sha256: [u8; 32],
}

/// One reason a parsed manifest cannot stand as a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A registered file whose digest differs from the source registry's.
    ShaDiffersFromRegistry {
        doc_id: String,
    },
    DuplicateInput {
        doc_id: String,
    },
    UrlNotHttp {
        doc_id: String,
    },
    /// An input published or retrieved after the information cut-off is outside the information set.
    InputAfterCutoff {
        doc_id: String,
    },
    AttestationDiffers,
    /// `author_role` is not a role id: lower-case words joined by `-`, at most 64 characters.
    AuthorRoleNotARoleId,
    ProofInverted,
    CutoffNotBeforeProof,
    /// The label disagrees with the proof's upper time.
    LabelInconsistent {
        label: Label,
    },
    ProofAfterDeadline,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShaDiffersFromRegistry { doc_id } => {
                write!(f, "{doc_id}: sha256 differs from the source registry")
            }
            Self::DuplicateInput { doc_id } => write!(f, "{doc_id}: listed twice"),
            Self::UrlNotHttp { doc_id } => write!(f, "{doc_id}: url is not http(s)"),
            Self::InputAfterCutoff { doc_id } => {
                write!(
                    f,
                    "{doc_id}: published or retrieved after the information cut-off"
                )
            }
            Self::AttestationDiffers => {
                write!(f, "the attestation differs from the registered text")
            }
            Self::AuthorRoleNotARoleId => {
                write!(
                    f,
                    "author_role is not a role id (lower-case words joined by '-')"
                )
            }
            Self::ProofInverted => write!(f, "the proof's lower time is after its upper time"),
            Self::CutoffNotBeforeProof => {
                write!(
                    f,
                    "the information cut-off is not before the proof's lower time"
                )
            }
            Self::LabelInconsistent { label } => {
                write!(
                    f,
                    "label {:?} disagrees with the proof time",
                    label.as_str()
                )
            }
            Self::ProofAfterDeadline => write!(f, "the proof's upper time is after the deadline"),
        }
    }
}

fn is_role_id(s: &str) -> bool {
    s.len() <= 64
        && s.split('-').all(|w| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        && s.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

/// Checks a parsed manifest against the source registry and, when one is given, a time proof. Each finding
/// stands on its own; an empty list is a valid record. The proof-dependent findings need `proof`.
#[must_use]
pub fn validate(
    m: &RecordManifest,
    registered: &[RegisteredDigest<'_>],
    proof: Option<ProofTimes>,
    dates: RecordDates,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let cutoff_day = m.information_cutoff_utc.div_euclid(SECONDS_PER_DAY);
    for (i, d) in m.inputs.iter().enumerate() {
        let doc_id = || d.doc_id.clone();
        if m.inputs[..i].iter().any(|e| e.doc_id == d.doc_id) {
            out.push(Finding::DuplicateInput { doc_id: doc_id() });
        }
        if !(d.url.starts_with("https://") || d.url.starts_with("http://")) {
            out.push(Finding::UrlNotHttp { doc_id: doc_id() });
        }
        if d.publication_day > cutoff_day || d.retrieved_utc > m.information_cutoff_utc {
            out.push(Finding::InputAfterCutoff { doc_id: doc_id() });
        }
        if registered
            .iter()
            .any(|r| r.file == d.doc_id && r.sha256 != d.sha256)
        {
            out.push(Finding::ShaDiffersFromRegistry { doc_id: doc_id() });
        }
    }
    if m.attestation != ATTESTATION {
        out.push(Finding::AttestationDiffers);
    }
    if !is_role_id(&m.author_role) {
        out.push(Finding::AuthorRoleNotARoleId);
    }
    if let Some(p) = proof {
        if p.lower_utc > p.upper_utc {
            out.push(Finding::ProofInverted);
        }
        if m.information_cutoff_utc >= p.lower_utc {
            out.push(Finding::CutoffNotBeforeProof);
        }
        let nowcast = p.upper_utc >= dates.nowcast_from;
        if nowcast != (m.label == Label::Nowcast) {
            out.push(Finding::LabelInconsistent { label: m.label });
        }
        if p.upper_utc > dates.proof_by {
            out.push(Finding::ProofAfterDeadline);
        }
    }
    out
}

fn exact<T, E>(r: Result<T, E>) -> Result<T, RecordError> {
    r.map_err(|_| RecordError::Overflow)
}

fn is_negative(x: Rat) -> bool {
    x.signum() < 0
}

/// The interval score at level α (Gneiting and Raftery 2007), exactly:
/// `(u − l) + (2/α)(l − y)·1{y < l} + (2/α)(y − u)·1{y > u}`. Refuses α outside (0, 1) and `l > u`; never clamps.
pub fn interval_score(lo: Rat, hi: Rat, alpha: Rat, y: Rat) -> Result<Rat, RecordError> {
    let one = Rat::from_integer(1);
    if alpha.signum() <= 0 || exact(alpha.checked_cmp(one))? != Ordering::Less {
        return Err(RecordError::AlphaOutOfDomain);
    }
    let width = exact(hi.checked_sub(lo))?;
    if is_negative(width) {
        return Err(RecordError::InvertedInterval);
    }
    let penalty = exact(Rat::from_integer(2).checked_div(alpha))?;
    let miss = if is_negative(exact(y.checked_sub(lo))?) {
        exact(lo.checked_sub(y))?
    } else if is_negative(exact(hi.checked_sub(y))?) {
        exact(y.checked_sub(hi))?
    } else {
        Rat::from_integer(0)
    };
    exact(width.checked_add(exact(penalty.checked_mul(miss))?))
}

/// A sample quantile estimator (Hyndman and Fan 1996).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estimator {
    /// Type 1, the inverse empirical CDF: always an observed value.
    InverseEcdf,
    /// Type 7, linear interpolation between order statistics (R's default).
    Linear,
}

fn floor(x: Rat) -> i128 {
    x.num().div_euclid(x.den())
}

fn index(i: i128) -> Result<usize, RecordError> {
    usize::try_from(i).map_err(|_| RecordError::Overflow)
}

/// The `p`-quantile of `sample`, exactly. Refuses an empty sample and `p` outside [0, 1].
pub fn quantile(sample: &[Rat], p: Rat, est: Estimator) -> Result<Rat, RecordError> {
    if sample.is_empty() {
        return Err(RecordError::EmptySample);
    }
    if is_negative(p) || exact(p.checked_cmp(Rat::from_integer(1)))? == Ordering::Greater {
        return Err(RecordError::LevelOutOfDomain);
    }
    let mut xs = sample.to_vec();
    let mut overflow = false;
    xs.sort_by(|a, b| {
        a.checked_cmp(*b).unwrap_or_else(|_| {
            overflow = true;
            Ordering::Equal
        })
    });
    if overflow {
        return Err(RecordError::Overflow);
    }
    let n = i128::try_from(xs.len()).map_err(|_| RecordError::Overflow)?;
    match est {
        Estimator::InverseEcdf => {
            // x_(k) with k = ceil(n p), and x_(0) read as x_(1).
            let np = exact(Rat::from_integer(n).checked_mul(p))?;
            let k = -floor(exact(np.checked_neg())?);
            Ok(xs[index(k.max(1) - 1)?])
        }
        Estimator::Linear => {
            // h = (n − 1) p on 0-based order statistics; q = x_j + (h − j)(x_j+1 − x_j), j = floor(h).
            let h = exact(Rat::from_integer(n - 1).checked_mul(p))?;
            let j = floor(h);
            let lo = xs[index(j)?];
            let Some(&hi) = xs.get(index(j + 1)?) else {
                return Ok(lo);
            };
            let g = exact(h.checked_sub(Rat::from_integer(j)))?;
            exact(lo.checked_add(exact(g.checked_mul(exact(hi.checked_sub(lo))?))?))
        }
    }
}

/// The run-rate persistence baseline's 80% interval (§21.6): `point × exp(q10, q90)` of the pooled log ratios
/// `log(Q_t+1 / Q_t)`. Under the inverse empirical CDF that is exactly `point × (q10, q90)` of the ratios, since
/// `exp` is increasing and the estimator returns an observed value. The linear estimator is refused
/// ([`RecordError::NotExact`]) rather than approximated.
pub fn persistence_interval(
    ratios: &[Rat],
    point: Rat,
    alpha: Rat,
    est: Estimator,
) -> Result<(Rat, Rat), RecordError> {
    let (lo, hi) = ratio_quantiles(ratios, alpha, est)?;
    Ok((exact(point.checked_mul(lo))?, exact(point.checked_mul(hi))?))
}

/// The ratios at levels α/2 and 1 − α/2 (q10 and q90 at α = 0.2), each an observed ratio under the inverse ECDF.
pub fn ratio_quantiles(
    ratios: &[Rat],
    alpha: Rat,
    est: Estimator,
) -> Result<(Rat, Rat), RecordError> {
    if est == Estimator::Linear {
        return Err(RecordError::NotExact);
    }
    if ratios.iter().any(|r| r.signum() <= 0) {
        return Err(RecordError::NonPositiveRatio);
    }
    let one = Rat::from_integer(1);
    if alpha.signum() <= 0 || exact(alpha.checked_cmp(one))? != Ordering::Less {
        return Err(RecordError::AlphaOutOfDomain);
    }
    let half = exact(alpha.checked_div(Rat::from_integer(2)))?;
    Ok((
        quantile(ratios, half, est)?,
        quantile(ratios, exact(one.checked_sub(half))?, est)?,
    ))
}

/// The persistence pool: every ratio `v[t+1] / v[t]` of consecutive quarters within each site's series. A pair
/// with either value `None` (not a positive printed number) is left out and counted, never imputed.
pub fn pooled_ratios(series: &[Vec<Option<Rat>>]) -> Result<(Vec<Rat>, usize), RecordError> {
    let (mut ratios, mut left_out) = (Vec::new(), 0);
    for s in series {
        for pair in s.windows(2) {
            match (pair[0], pair[1]) {
                (Some(a), Some(b)) if a.signum() > 0 && b.signum() > 0 => {
                    ratios.push(exact(b.checked_div(a))?);
                }
                _ => left_out += 1,
            }
        }
    }
    Ok((ratios, left_out))
}

/// One row of `forecasts.csv`: a live site's interval for one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastRow {
    pub site: String,
    pub target: String,
    pub point: Rat,
    pub lo: Rat,
    pub hi: Rat,
}

/// The header `forecasts.csv` must carry under these rules: `site,target,point,p10,p90,uncalibrated` at α = 0.2.
pub fn forecasts_header(rules: &RecordRules) -> Result<String, RecordError> {
    let lo = exact(rules.alpha.checked_mul(Rat::from_integer(50)))?;
    if lo.den() != 1 {
        return Err(RecordError::AlphaOutOfDomain);
    }
    let lo = lo.num();
    Ok(format!(
        "site,target,point,p{lo},p{},uncalibrated",
        100 - lo
    ))
}

/// Parses `forecasts.csv` against the rules (BD-334.S3): the rules' header; then exactly one row for each live
/// site × target and none for a `not_published` site or anything outside the universe; decimals with
/// 0 ≤ lower ≤ point ≤ upper; and `uncalibrated` = `true`, since one period cannot calibrate (plan §21.6).
/// Lines end in `\n`; no field holds a comma, quote or surrounding space.
pub fn parse_forecasts(csv: &[u8], rules: &RecordRules) -> Result<Vec<ForecastRow>, RecordError> {
    let bad = |line: usize, reason: String| RecordError::Forecast { line, reason };
    let text = std::str::from_utf8(csv).map_err(|_| bad(0, "not UTF-8".to_string()))?;
    let body = text
        .strip_suffix('\n')
        .ok_or_else(|| bad(text.lines().count(), "the last line has no \\n".to_string()))?;
    let mut lines = body.split('\n');
    let header = forecasts_header(rules)?;
    if lines.next() != Some(header.as_str()) {
        return Err(bad(1, format!("the header must be {header:?}")));
    }
    let mut rows: Vec<ForecastRow> = Vec::new();
    for (k, line) in lines.enumerate() {
        let n = k + 2;
        let cells: Vec<&str> = line.split(',').collect();
        let [site, target, point, lo, hi, flag] = cells[..] else {
            return Err(bad(n, format!("{} fields, not 6", cells.len())));
        };
        match rules.sites.iter().find(|s| s.name == site) {
            None => return Err(bad(n, format!("{site:?} is not a site of the rules"))),
            Some(s) if s.status == SiteStatus::NotPublished => {
                return Err(bad(
                    n,
                    format!("{site} is not_published: it is listed, not forecast"),
                ));
            }
            Some(_) => {}
        }
        if !rules.targets.iter().any(|t| t.id == target) {
            return Err(bad(n, format!("{target:?} is not a target of the rules")));
        }
        if rows.iter().any(|r| r.site == site && r.target == target) {
            return Err(bad(n, format!("a second {target} forecast for {site}")));
        }
        if flag != "true" {
            return Err(bad(n, "uncalibrated must be true".to_string()));
        }
        let number = |s: &str| {
            Rat::from_decimal_str(s)
                .ok()
                .filter(|x| x.signum() >= 0)
                .ok_or_else(|| bad(n, format!("{s:?} is not a non-negative decimal")))
        };
        let (point, lo, hi) = (number(point)?, number(lo)?, number(hi)?);
        let le = |a: Rat, b: Rat| exact(a.checked_cmp(b)).map(|o| o != Ordering::Greater);
        if !(le(lo, point)? && le(point, hi)?) {
            return Err(bad(
                n,
                "the bounds must satisfy lower <= point <= upper".to_string(),
            ));
        }
        rows.push(ForecastRow {
            site: site.to_string(),
            target: target.to_string(),
            point,
            lo,
            hi,
        });
    }
    for s in rules.sites.iter().filter(|s| s.status == SiteStatus::Live) {
        for t in &rules.targets {
            if !rows.iter().any(|r| r.site == s.name && r.target == t.id) {
                return Err(RecordError::MissingForecast {
                    site: s.name.clone(),
                    target: t.id.clone(),
                });
            }
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    fn i(n: i128) -> Rat {
        Rat::from_integer(n)
    }

    const RULES_HEX: &str = "9d7934e0aaff5f1479768c5fc45262f852a5fa207718328f65a34295794b6cb3";

    fn manifest_text(cutoff: &str, label: &str, attestation: &str) -> String {
        format!(
            "record_id = \"FM-SYNTHETIC-v1\"\n\
             author_role = \"public-lane-author\"\n\
             information_cutoff_utc = \"{cutoff}\"\n\
             label = \"{label}\"\n\
             contact = \"pre-contact\"\n\
             attestation = \"{attestation}\"\n\
             rules_sha256 = \"{RULES_HEX}\"\n\
             \n\
             [[input]]\n\
             doc_id = \"Synthetic_Mine_Stats.xlsx\"\n\
             url = \"https://example.invalid/stats.xlsx\"\n\
             publication_date = \"2026-08-10\"\n\
             retrieved_utc = \"2026-09-01T12:00:00Z\"\n\
             bytes = 1_524\n\
             sha256 = \"{}\"\n",
            "ab".repeat(32)
        )
    }

    fn manifest(cutoff: &str, label: &str) -> RecordManifest {
        parse_manifest(&manifest_text(cutoff, label, ATTESTATION)).expect("synthetic manifest")
    }

    fn proof(lower: &str, upper: &str) -> ProofTimes {
        ProofTimes {
            lower_utc: parse_utc(lower).expect("lower"),
            upper_utc: parse_utc(upper).expect("upper"),
        }
    }

    // Known answers computed outside this code: `printf 'FMREC1\n2\na\n2\nb\n' | shasum -a 256`, and so on.
    #[test]
    fn test_record_frame_known_answers() {
        assert_eq!(
            record_frame(b"a\n", b"b\n"),
            b"FMREC1\n2\na\n2\nb\n".to_vec()
        );
        assert_eq!(
            record_digest(b"a\n", b"b\n").to_hex(),
            "bbe54b81339e3d5b8b9a462cdfc9025c3808d0c653778d636193dab10551b407"
        );
        assert_eq!(
            record_digest(b"a\n", b"c\n").to_hex(),
            "9edc5c33686ad6c6ba1b3238c6c56b5bd9685efcb697ab6a757cda98905b1116"
        );
        assert_eq!(
            record_digest(b"", b"").to_hex(),
            "84f4528ededcaf3065ac051b101d6f32310d4114216a306e02a5e48b3790d6bf"
        );
    }

    #[test]
    fn test_rules_frame_known_answer() {
        assert_eq!(rules_frame(b"r\n"), b"FMRUL1\n2\nr\n".to_vec());
        assert_eq!(rules_digest(b"r\n").to_hex(), RULES_HEX);
    }

    #[test]
    fn test_one_byte_change_moves_digest() {
        let (m, c) = (b"manifest\n".to_vec(), b"site,point\nA,1\n".to_vec());
        let base = record_digest(&m, &c);
        for k in 0..m.len() {
            let mut m2 = m.clone();
            m2[k] ^= 1;
            assert_ne!(record_digest(&m2, &c), base, "manifest byte {k}");
        }
        for k in 0..c.len() {
            let mut c2 = c.clone();
            c2[k] ^= 1;
            assert_ne!(record_digest(&m, &c2), base, "csv byte {k}");
        }
        // A byte moved from the end of one file to the start of the other moves the digest too.
        assert_ne!(record_digest(b"ab", b"c"), record_digest(b"a", b"bc"));
    }

    #[test]
    fn test_interval_score_worked_examples() {
        let (lo, hi, a) = (i(40), i(50), r(1, 5));
        assert_eq!(interval_score(lo, hi, a, i(52)), Ok(i(30)));
        assert_eq!(interval_score(lo, hi, a, i(45)), Ok(i(10)));
        assert_eq!(interval_score(lo, hi, a, i(40)), Ok(i(10)));
        assert_eq!(interval_score(lo, hi, a, i(50)), Ok(i(10)));
        assert_eq!(interval_score(lo, hi, r(1, 2), i(52)), Ok(i(18)));
        assert_eq!(interval_score(lo, hi, a, i(37)), Ok(i(40)));
        // Exact where floating point is not: 0.1-wide interval, outcome 0.3 above it, alpha 0.2.
        assert_eq!(
            interval_score(r(1, 10), r(2, 10), r(2, 10), r(5, 10)),
            Ok(r(31, 10))
        );
    }

    #[test]
    fn test_interval_score_refuses_alpha_edges() {
        for a in [i(0), i(1), i(-1), r(3, 2)] {
            assert_eq!(
                interval_score(i(40), i(50), a, i(45)),
                Err(RecordError::AlphaOutOfDomain)
            );
        }
    }

    #[test]
    fn test_interval_score_refuses_l_gt_u() {
        assert_eq!(
            interval_score(i(50), i(40), r(1, 5), i(45)),
            Err(RecordError::InvertedInterval)
        );
    }

    #[test]
    fn test_interval_score_agrees_with_winkler() {
        // The ledger's float rule and the record's exact rule are the same formula.
        for y in [37_i32, 40, 45, 50, 52] {
            let exact = interval_score(i(40), i(50), r(1, 5), i(y.into())).expect("score");
            let float =
                crate::winkler_interval_score(40.0, 50.0, 0.2, f64::from(y)).expect("winkler");
            assert!((exact.to_f64() - float).abs() < 1e-9, "y = {y}");
        }
    }

    #[test]
    fn test_no_clamp_remains_in_fmine_calib() {
        for (name, src) in [
            ("lib.rs", include_str!("lib.rs")),
            ("record.rs", include_str!("record.rs")),
        ] {
            let needle = [".cla", "mp("].concat();
            assert!(!src.contains(&needle), "{name} clamps");
        }
    }

    #[test]
    fn test_persistence_type7_known_quantiles() {
        // Log ratios k/10 for k = -5..=5: type 7 gives q10 = -0.4 and q90 = 0.4 (hand-derived: h = 1 and 9).
        let sample: Vec<Rat> = (-5..=5).rev().map(|k| r(k, 10)).collect();
        assert_eq!(
            quantile(&sample, r(1, 10), Estimator::Linear),
            Ok(r(-4, 10))
        );
        assert_eq!(quantile(&sample, r(9, 10), Estimator::Linear), Ok(r(4, 10)));
        // The two estimators differ off the order statistics: [1, 2, 3, 4] at p = 0.5.
        let s4 = [i(4), i(1), i(3), i(2)];
        assert_eq!(quantile(&s4, r(1, 2), Estimator::Linear), Ok(r(5, 2)));
        assert_eq!(quantile(&s4, r(1, 2), Estimator::InverseEcdf), Ok(i(2)));
        assert_eq!(quantile(&s4, i(0), Estimator::InverseEcdf), Ok(i(1)));
        assert_eq!(quantile(&s4, i(1), Estimator::Linear), Ok(i(4)));
        assert_eq!(
            quantile(&[], r(1, 2), Estimator::Linear),
            Err(RecordError::EmptySample)
        );
        assert_eq!(
            quantile(&s4, r(11, 10), Estimator::Linear),
            Err(RecordError::LevelOutOfDomain)
        );
    }

    #[test]
    fn test_persistence_inverse_ecdf_is_exact() {
        // Ten ratios 0.91..=1.00: n p = 1 and 9, so q10 = 0.91 and q90 = 0.99; point 200 gives [182, 198].
        let ratios: Vec<Rat> = (91..=100).map(|k| r(k, 100)).collect();
        let a = r(1, 5);
        assert_eq!(
            persistence_interval(&ratios, i(200), a, Estimator::InverseEcdf),
            Ok((i(182), i(198)))
        );
        // α = 0.4: levels 0.2 and 0.8, n p = 2 and 8, so the 2nd and 8th ratios.
        assert_eq!(
            persistence_interval(&ratios, i(200), r(2, 5), Estimator::InverseEcdf),
            Ok((i(184), i(196)))
        );
        assert_eq!(
            persistence_interval(&ratios, i(200), a, Estimator::Linear),
            Err(RecordError::NotExact)
        );
        assert_eq!(
            persistence_interval(&[i(1), i(0)], i(200), a, Estimator::InverseEcdf),
            Err(RecordError::NonPositiveRatio)
        );
        for alpha in [i(0), i(1)] {
            assert_eq!(
                persistence_interval(&ratios, i(200), alpha, Estimator::InverseEcdf),
                Err(RecordError::AlphaOutOfDomain)
            );
        }
    }

    /// Two sites over four quarters: A gives 110/100, 121/110 and 133.1/121 (each 1.1); B's gap leaves its two
    /// pairs out, and a zero leaves a pair out too, both counted.
    #[test]
    fn test_pooled_ratios_leave_out_and_count() {
        let a = vec![Some(i(100)), Some(i(110)), Some(i(121)), Some(r(1331, 10))];
        let b = vec![Some(i(50)), None, Some(i(40)), Some(i(0))];
        let (ratios, left_out) = pooled_ratios(&[a, b]).expect("pool");
        assert_eq!(ratios, vec![r(11, 10); 3]);
        assert_eq!(left_out, 3);
        assert_eq!(pooled_ratios(&[vec![Some(i(1))]]), Ok((vec![], 0)));
    }

    #[test]
    fn test_quarters() {
        let q = |s| Quarter::parse(s).expect("quarter");
        assert_eq!(q("2024Q4").next(), q("2025Q1"));
        assert_eq!(q("2024Q1").through(q("2026Q2")).len(), 10);
        assert!(q("2025Q1").through(q("2024Q4")).is_empty());
        assert_eq!(q("2026Q3").to_string(), "2026Q3");
        for bad in [
            "2026Q0", "2026Q5", "26Q1", "2026q1", "2026-Q1", "2026Q", "Q1",
        ] {
            assert_eq!(Quarter::parse(bad), None, "{bad}");
        }
    }

    fn forecasts_csv(rows: &[&str]) -> Vec<u8> {
        let mut s = String::from("site,target,point,p10,p90,uncalibrated\n");
        for row in rows {
            s.push_str(row);
            s.push('\n');
        }
        s.into_bytes()
    }

    #[test]
    fn test_forecasts_parse_against_the_rules() {
        let rules = parse_rules(&rules_text()).expect("rules");
        assert_eq!(
            forecasts_header(&rules).as_deref(),
            Ok("site,target,point,p10,p90,uncalibrated")
        );
        let ok = forecasts_csv(&["Synthetic A,production,100,90.5,1.1e2,true"]);
        assert_eq!(
            parse_forecasts(&ok, &rules),
            Ok(vec![ForecastRow {
                site: "Synthetic A".to_string(),
                target: "production".to_string(),
                point: i(100),
                lo: r(181, 2),
                hi: i(110),
            }])
        );
        let line = |e| match e {
            Err(RecordError::Forecast { line, .. }) => line,
            other => panic!("{other:?}"),
        };
        let a = "Synthetic A,production,100,90,110,true";
        for (rows, at) in [
            (vec![a, a], 3),
            (vec!["Synthetic B,production,1,1,1,true"], 2),
            (vec!["Synthetic C,production,1,1,1,true"], 2),
            (vec!["Synthetic A,unit_cost,1,1,1,true"], 2),
            (vec!["Synthetic A,production,100,90,110,false"], 2),
            (vec!["Synthetic A,production,100,101,110,true"], 2),
            (vec!["Synthetic A,production,100,90,99,true"], 2),
            (vec!["Synthetic A,production,-1,-2,0,true"], 2),
            (vec!["Synthetic A,production,1e,1,1,true"], 2),
            (vec!["Synthetic A,production,100,90,110"], 2),
            (vec!["Synthetic A, production,100,90,110,true"], 2),
        ] {
            assert_eq!(
                line(parse_forecasts(&forecasts_csv(&rows), &rules)),
                at,
                "{rows:?}"
            );
        }
        assert_eq!(
            parse_forecasts(&forecasts_csv(&[]), &rules),
            Err(RecordError::MissingForecast {
                site: "Synthetic A".to_string(),
                target: "production".to_string()
            })
        );
        let crlf = String::from_utf8(ok.clone())
            .expect("utf8")
            .replace('\n', "\r\n");
        assert!(parse_forecasts(crlf.as_bytes(), &rules).is_err());
        assert_eq!(line(parse_forecasts(&ok[..ok.len() - 1], &rules)), 2);
        let other_header = String::from_utf8(ok)
            .expect("utf8")
            .replace("p10,p90", "lo,hi");
        assert_eq!(line(parse_forecasts(other_header.as_bytes(), &rules)), 1);
    }

    #[test]
    fn test_utc_round_trip_and_fy2026_dates() {
        // Epoch seconds computed outside this code (Python's datetime, UTC).
        assert_eq!(parse_utc("2026-11-09T11:00:00Z"), Some(1_794_222_000));
        assert_eq!(FY2026_DATES.nowcast_from, 1_794_222_000);
        assert_eq!(FY2026_DATES.proof_by, 1_798_761_599);
        assert_eq!(format_utc(1_798_761_599), "2026-12-31T23:59:59Z");
        for bad in [
            "2026-11-09T11:00:00",
            "2026-11-09 11:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-11-09T24:00:00Z",
            "2026-11-09T11:00:00+00:00",
        ] {
            assert_eq!(parse_utc(bad), None, "{bad}");
        }
        assert_eq!(
            parse_date("2026-08-10").map(|d| d * 86_400),
            parse_utc("2026-08-10T00:00:00Z")
        );
    }

    #[test]
    fn test_manifest_parses() {
        let m = manifest("2026-10-01T00:00:00Z", "forecast");
        assert_eq!(m.label, Label::Forecast);
        assert_eq!(m.contact, Contact::PreContact);
        assert_eq!(m.inputs.len(), 1);
        assert_eq!(m.inputs[0].bytes, 1_524);
        assert_eq!(m.rules_sha256, parse_hex32(RULES_HEX).expect("hex"));
        assert!(validate(&m, &[], None, FY2026_DATES).is_empty());
    }

    #[test]
    fn test_unknown_key_refused() {
        let text = manifest_text("2026-10-01T00:00:00Z", "forecast", ATTESTATION);
        let top = text.replacen("label =", "extra = \"x\"\nlabel =", 1);
        assert!(matches!(
            parse_manifest(&top),
            Err(RecordError::UnknownKey { key, .. }) if key == "extra"
        ));
        let row = format!("{text}notes = \"x\"\n");
        assert!(matches!(
            parse_manifest(&row),
            Err(RecordError::UnknownKey { key, .. }) if key == "notes"
        ));
        let table = format!("{text}[[baseline]]\nid = \"x\"\n");
        assert!(matches!(
            parse_manifest(&table),
            Err(RecordError::UnknownTable { table, .. }) if table == "baseline"
        ));
        // Outside the subset: a boolean, which is why the flags are words.
        let boolean = text.replacen("label = \"forecast\"", "label = false", 1);
        assert!(matches!(
            parse_manifest(&boolean),
            Err(RecordError::Syntax { .. })
        ));
    }

    #[test]
    fn test_missing_attestation_refused() {
        let text = manifest_text("2026-10-01T00:00:00Z", "forecast", ATTESTATION);
        let without = text.replacen(&format!("attestation = \"{ATTESTATION}\"\n"), "", 1);
        assert_eq!(
            parse_manifest(&without),
            Err(RecordError::Missing {
                line: 0,
                key: "attestation"
            })
        );
        let reworded = parse_manifest(&manifest_text(
            "2026-10-01T00:00:00Z",
            "forecast",
            "No private information was used.",
        ))
        .expect("parses");
        assert_eq!(
            validate(&reworded, &[], None, FY2026_DATES),
            vec![Finding::AttestationDiffers]
        );
    }

    #[test]
    fn test_manifest_sha_mismatch_refused() {
        let m = manifest("2026-10-01T00:00:00Z", "forecast");
        let same = RegisteredDigest {
            file: "Synthetic_Mine_Stats.xlsx",
            sha256: [0xab; 32],
        };
        let other = RegisteredDigest {
            sha256: [0xac; 32],
            ..same
        };
        assert!(validate(&m, &[same], None, FY2026_DATES).is_empty());
        assert_eq!(
            validate(&m, &[other], None, FY2026_DATES),
            vec![Finding::ShaDiffersFromRegistry {
                doc_id: "Synthetic_Mine_Stats.xlsx".to_string()
            }]
        );
    }

    #[test]
    fn test_cutoff_after_proof_refused() {
        let m = manifest("2026-10-01T00:00:00Z", "forecast");
        let ok = proof("2026-10-30T12:00:00Z", "2026-10-30T12:00:01Z");
        assert!(validate(&m, &[], Some(ok), FY2026_DATES).is_empty());
        let early = proof("2026-09-30T12:00:00Z", "2026-09-30T12:00:01Z");
        assert_eq!(
            validate(&m, &[], Some(early), FY2026_DATES),
            vec![Finding::CutoffNotBeforeProof]
        );
        // An input retrieved after the cut-off is outside the information set.
        let late = manifest("2026-08-31T00:00:00Z", "forecast");
        assert_eq!(
            validate(&late, &[], None, FY2026_DATES),
            vec![Finding::InputAfterCutoff {
                doc_id: "Synthetic_Mine_Stats.xlsx".to_string()
            }]
        );
    }

    #[test]
    fn test_hashed_after_q3_flag_consistency() {
        let forecast = manifest("2026-10-01T00:00:00Z", "forecast");
        let nowcast = manifest("2026-10-01T00:00:00Z", "nowcast");
        let before = proof("2026-11-09T10:59:58Z", "2026-11-09T10:59:59Z");
        let straddles = proof("2026-11-09T10:59:59Z", "2026-11-09T11:00:00Z");
        let after = proof("2026-12-01T00:00:00Z", "2026-12-01T00:00:01Z");
        let check = |m: &RecordManifest, p| validate(m, &[], Some(p), FY2026_DATES);
        assert!(check(&forecast, before).is_empty());
        assert!(check(&nowcast, straddles).is_empty());
        assert!(check(&nowcast, after).is_empty());
        let wrong = |label| vec![Finding::LabelInconsistent { label }];
        assert_eq!(check(&nowcast, before), wrong(Label::Nowcast));
        assert_eq!(check(&forecast, straddles), wrong(Label::Forecast));
        let late = proof("2027-01-01T00:00:00Z", "2027-01-01T00:00:01Z");
        assert_eq!(check(&nowcast, late), vec![Finding::ProofAfterDeadline]);
    }

    #[test]
    fn test_author_role_must_be_a_role_id() {
        let text = manifest_text("2026-10-01T00:00:00Z", "forecast", ATTESTATION);
        for (role, ok) in [
            ("public-lane-author", true),
            ("author2", true),
            ("Given Family", false),
            ("-author", false),
            ("author-", false),
            ("Author", false),
        ] {
            let m = parse_manifest(&text.replacen("public-lane-author", role, 1)).expect("parses");
            let found =
                validate(&m, &[], None, FY2026_DATES).contains(&Finding::AuthorRoleNotARoleId);
            assert_eq!(found, !ok, "{role}");
        }
    }

    fn rules_text() -> String {
        "rules_id = \"R-v1\"\nrecord_id = \"FM-SYNTHETIC-v1\"\nissuer = \"synthetic\"\nalpha = \"0.2\"\n\
         interval_score = \"IS\"\nquantile_estimator = \"inverse_ecdf\"\nrho_low = \"0\"\nrho_high = \"0.5\"\n\
         coverage_level = \"0.95\"\nreport = \"n\"\ncalibration_claim_min_n = 35\ncalibration_claim = \"none\"\n\
         nowcast_from_utc = \"2026-11-09T11:00:00Z\"\nproof_by_utc = \"2026-12-31T23:59:59Z\"\n\
         outcome_vintage = \"first\"\nexcluded_rows = \"roll-ups\"\n\n\
         [[target]]\nid = \"production\"\nquantity = \"koz\"\nscored_quantity = \"Q4\"\n\
         field = \"au_prod_100_koz\"\noutcome = \"fy_minus_9m\"\n\n\
         [[basis_rule]]\nid = \"divestment\"\nrule = \"listed\"\n\n\
         [[baseline]]\nid = \"run_rate_persistence\"\ntarget = \"production\"\nrule = \"q3\"\nsource = \"wb\"\n\
         pool_from = \"2024Q1\"\npool_through = \"2026Q2\"\npoint_quarter = \"2026Q3\"\n\n\
         [[site]]\nname = \"Synthetic A\"\nstatus = \"live\"\nlocator = \"S!A1\"\n\n\
         [[site]]\nname = \"Synthetic B\"\nstatus = \"not_published\"\nlocator = \"S!A2\"\nreason = \"sold\"\n"
            .to_string()
    }

    #[test]
    fn test_rules_parse() {
        let rules = parse_rules(&rules_text()).expect("synthetic rules");
        assert_eq!(rules.alpha, r(1, 5));
        assert_eq!(rules.estimator, Estimator::InverseEcdf);
        assert_eq!((rules.rho_low, rules.rho_high), (i(0), r(1, 2)));
        assert_eq!(rules.coverage_level, r(19, 20));
        assert_eq!(rules.calibration_claim_min_n, 35);
        assert_eq!(rules.dates, FY2026_DATES);
        assert_eq!(rules.baselines[0].target, "production");
        let q = |s| Quarter::parse(s).expect("quarter");
        assert_eq!(
            rules.baselines[0].kind,
            BaselineKind::Persistence {
                pool_from: q("2024Q1"),
                pool_through: q("2026Q2"),
                point: q("2026Q3"),
            }
        );
        assert_eq!(
            (rules.targets[0].field.as_str(), rules.targets[0].outcome),
            ("au_prod_100_koz", Outcome::FyMinus9m)
        );
        assert_eq!(rules.sites[0].reason, None);
        assert_eq!(rules.sites[1].status, SiteStatus::NotPublished);
        assert_eq!(rules.sites[1].reason.as_deref(), Some("sold"));
    }

    #[test]
    fn test_rules_refusals() {
        let base = rules_text();
        let refused = |text: String| parse_rules(&text).expect_err("refused");
        let with = |from: &str, to: &str| base.replacen(from, to, 1);
        assert!(matches!(
            refused(with("\"q3\"", "\"OPEN: which quarter\"")),
            RecordError::OpenMarker { .. }
        ));
        assert!(parse_rules(&with("\"q3\"", "\"OpenTimestamps, REOPENED\"")).is_ok());
        assert!(matches!(
            refused(with("target = \"production\"", "target = \"cost\"")),
            RecordError::UnknownTarget { target, .. } if target == "cost"
        ));
        assert!(matches!(
            refused(with("Synthetic B", "Synthetic A")),
            RecordError::DuplicateId { id, .. } if id == "Synthetic A"
        ));
        assert!(matches!(
            refused(with(
                "locator = \"S!A1\"",
                "locator = \"S!A1\"\nreason = \"x\""
            )),
            RecordError::BadValue { key: "reason", .. }
        ));
        assert!(matches!(
            refused(with("reason = \"sold\"\n", "")),
            RecordError::Missing { key: "reason", .. }
        ));
        for alpha in ["0", "1", "-0.2", "0.2x"] {
            assert!(matches!(
                refused(with("alpha = \"0.2\"", &format!("alpha = \"{alpha}\""))),
                RecordError::BadValue { key: "alpha", .. }
            ));
        }
        assert!(matches!(
            refused(with("rho_low = \"0\"", "rho_low = \"0.6\"")),
            RecordError::BadValue {
                key: "rho_high",
                ..
            }
        ));
        assert!(matches!(
            refused(with("id = \"run_rate_persistence\"", "id = \"persistence\"")),
            RecordError::UnknownBaseline { id, .. } if id == "persistence"
        ));
        for (from, to) in [
            ("point_quarter = \"2026Q3\"", "point_quarter = \"2026Q2\""),
            ("pool_through = \"2026Q2\"", "pool_through = \"2024Q1\""),
        ] {
            assert!(matches!(
                refused(with(from, to)),
                RecordError::BadValue {
                    key: "point_quarter",
                    ..
                }
            ));
        }
        assert!(matches!(
            refused(with("pool_from = \"2024Q1\"", "pool_from = \"2024Q5\"")),
            RecordError::BadValue {
                key: "pool_from",
                ..
            }
        ));
        assert!(matches!(
            refused(with("pool_from = \"2024Q1\"\n", "")),
            RecordError::Missing {
                key: "pool_from",
                ..
            }
        ));
        assert!(matches!(
            refused(with("outcome = \"fy_minus_9m\"", "outcome = \"fy\"")),
            RecordError::BadValue { key: "outcome", .. }
        ));
        assert!(matches!(
            refused(with(
                "field = \"au_prod_100_koz\"",
                "field = \"Gold produced\""
            )),
            RecordError::BadValue { key: "field", .. }
        ));
        let no_baseline = format!(
            "{base}\n[[target]]\nid = \"unit_cost\"\nquantity = \"$/oz\"\nscored_quantity = \"Q4\"\n\
             field = \"aisc_usd_per_oz\"\noutcome = \"q4_qtd\"\n"
        );
        assert_eq!(
            refused(no_baseline),
            RecordError::TargetWithoutBaseline {
                target: "unit_cost".to_string()
            }
        );
        assert!(matches!(
            refused(format!("{base}\n[[input]]\ndoc_id = \"x\"\n")),
            RecordError::UnknownTable { .. }
        ));
        assert_eq!(
            refused(with("report = \"n\"\n", "")),
            RecordError::Missing {
                line: 0,
                key: "report"
            }
        );
    }
}
