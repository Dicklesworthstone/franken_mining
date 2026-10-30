//! The public forecast record (BD-334, plan §21.6): a manifest hashed together with the forecasts, so that they
//! can be scored later without judgment, and a rules file hashed and stamped before any forecast exists.
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
use fmine_numerics::exact_feas::Rat;
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
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { line, reason } => write!(f, "line {line}: {reason}"),
            Self::UnknownTable { line, table } => {
                write!(f, "line {line}: unknown table [[{table}]]")
            }
            Self::UnknownKey { line, key } => write!(f, "line {line}: unknown key {key:?}"),
            Self::Missing { line: 0, key } => write!(f, "the manifest has no {key}"),
            Self::Missing { line, key } => {
                write!(f, "the [[input]] row on line {line} has no {key}")
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
    est: Estimator,
) -> Result<(Rat, Rat), RecordError> {
    if est == Estimator::Linear {
        return Err(RecordError::NotExact);
    }
    if ratios.iter().any(|r| r.signum() <= 0) {
        return Err(RecordError::NonPositiveRatio);
    }
    let level = |n| exact(Rat::new(n, 10));
    let q10 = quantile(ratios, level(1)?, est)?;
    let q90 = quantile(ratios, level(9)?, est)?;
    Ok((
        exact(point.checked_mul(q10))?,
        exact(point.checked_mul(q90))?,
    ))
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
        assert_eq!(
            persistence_interval(&ratios, i(200), Estimator::InverseEcdf),
            Ok((i(182), i(198)))
        );
        assert_eq!(
            persistence_interval(&ratios, i(200), Estimator::Linear),
            Err(RecordError::NotExact)
        );
        assert_eq!(
            persistence_interval(&[i(1), i(0)], i(200), Estimator::InverseEcdf),
            Err(RecordError::NonPositiveRatio)
        );
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
}
