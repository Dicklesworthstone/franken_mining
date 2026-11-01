//! Frozen semantic targets for registered public-cell resolution; no source I/O or time authentication.
//! Determinism: BitwisePortable

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::Refusal;
use fmine_time::CivilDate;

use super::{CohortSpec, ForecastPayload, FrozenForecast, domain, frozen, late};

const RESERVED: &str = "fmine.public-resolution";
pub const PUBLIC_RESOLUTION_PREFIX: &str = "fmine.public-resolution.v1:";
pub const PUBLIC_RESOLUTION_V2_PREFIX: &str = "fmine.public-resolution.v2:";
const MAX_MEMBERS: usize = 4096;

/// Whether later registered disclosures may append a separate outcome vintage. The first outcome always
/// comes from the original release; this permission never allows a comparative to fill a missing original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicVintagePolicy {
    OriginalOnly,
    RegisteredComparatives,
}

impl PublicVintagePolicy {
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        match value {
            "original_only" => Ok(Self::OriginalOnly),
            "registered_comparatives" => Ok(Self::RegisteredComparatives),
            _ => Err(domain("unsupported public outcome-vintage policy")),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OriginalOnly => "original_only",
            Self::RegisteredComparatives => "registered_comparatives",
        }
    }

    /// Validate the frozen calendar-release permission. The adapter must additionally authenticate the
    /// selected source and require an existing original outcome and the latest restatement predecessor.
    pub fn validate_release(
        self,
        target: PublicPeriod,
        release: &str,
        restatement: bool,
    ) -> Result<(), Refusal> {
        target.end_ns()?;
        let selected = PublicPeriod::parse(release)?;
        if !matches!(selected, PublicPeriod::Quarter { .. }) || selected.label() != release {
            return Err(domain(
                "a public source release must be a canonical YYYYQ1..4 quarter",
            ));
        }
        let original = target.original_release();
        if !restatement && release != original {
            return Err(frozen(
                "the first public outcome must use its frozen original release",
            ));
        }
        if restatement && (self != Self::RegisteredComparatives || release <= original.as_str()) {
            return Err(frozen(
                "the frozen policy does not permit this comparative restatement",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicMetric {
    ProductionKoz,
    AiscUsdPerOz,
}

impl PublicMetric {
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        match value {
            "production_koz" => Ok(Self::ProductionKoz),
            "aisc_usd_per_oz" => Ok(Self::AiscUsdPerOz),
            _ => Err(domain("unsupported public-resolution metric")),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProductionKoz => "production_koz",
            Self::AiscUsdPerOz => "aisc_usd_per_oz",
        }
    }

    pub const fn unit(self) -> &'static str {
        match self {
            Self::ProductionKoz => "koz",
            Self::AiscUsdPerOz => "usd_per_oz",
        }
    }

    /// Exact original-release line label; AISC means issuer-reported cost per sold gold ounce.
    pub const fn field(self, ownership: PublicOwnership) -> &'static str {
        match (self, ownership) {
            (Self::ProductionKoz, PublicOwnership::Full) => "summary:au_prod_100_koz",
            (Self::ProductionKoz, PublicOwnership::Attributable { .. }) => {
                "summary:au_prod_attr_koz"
            }
            (Self::AiscUsdPerOz, _) => "summary:aisc_usd_per_oz",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicPeriod {
    Year(u16),
    Quarter { year: u16, quarter: u8 },
}

impl PublicPeriod {
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        let bad = || domain("public resolution requires a calendar YYYYFY or YYYYQ1..4 period");
        let bytes = value.as_bytes();
        if bytes.len() != 6 || !bytes[..4].iter().all(u8::is_ascii_digit) {
            return Err(bad());
        }
        let year = value[..4].parse().map_err(|_| bad())?;
        let period = match &bytes[4..] {
            b"FY" => Self::Year(year),
            [b'Q', q @ b'1'..=b'4'] => Self::Quarter {
                year,
                quarter: q - b'0',
            },
            _ => return Err(bad()),
        };
        period.end_ns()?;
        Ok(period)
    }

    pub fn label(self) -> String {
        match self {
            Self::Year(year) => format!("{year:04}FY"),
            Self::Quarter { year, quarter } => format!("{year:04}Q{quarter}"),
        }
    }

    pub fn original_release(self) -> String {
        match self {
            Self::Year(year) => format!("{year:04}Q4"),
            Self::Quarter { .. } => self.label(),
        }
    }

    /// Last nanosecond of the calendar period, not its publication or availability time.
    pub fn end_ns(self) -> Result<i64, Refusal> {
        let (year, quarter) = match self {
            Self::Year(year) => (year, 4),
            Self::Quarter { year, quarter } if (1..=4).contains(&quarter) => (year, quarter),
            _ => return Err(domain("invalid public-resolution quarter")),
        };
        let (next_year, next_month) = if quarter == 4 {
            (i32::from(year) + 1, 1)
        } else {
            (i32::from(year), quarter * 3 + 1)
        };
        CivilDate::new(next_year, next_month, 1)
            .ok()
            .and_then(|date| date.to_epoch_days().checked_mul(86_400_000_000_000))
            .and_then(|ns| ns.checked_sub(1))
            .ok_or_else(|| domain("public-resolution period exceeds the UTC nanosecond domain"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicOwnership {
    Full,
    Attributable { num: u32, den: u32 },
}

impl PublicOwnership {
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        if value == "100%" {
            return Ok(Self::Full);
        }
        let bad = || domain("public ownership must be 100% or attributable:positive_fraction<=1");
        let (num, den) = value
            .strip_prefix("attributable:")
            .and_then(|fraction| fraction.split_once('/'))
            .ok_or_else(bad)?;
        let num: u32 = num.parse().map_err(|_| bad())?;
        let den: u32 = den.parse().map_err(|_| bad())?;
        if num == 0 || den == 0 || num > den {
            return Err(bad());
        }
        let divisor = gcd(num, den);
        Ok(Self::Attributable {
            num: num / divisor,
            den: den / divisor,
        })
    }

    pub fn label(self) -> String {
        match self {
            Self::Full => "100%".into(),
            Self::Attributable { num, den } => format!("attributable:{num}/{den}"),
        }
    }

    fn validate(self) -> Result<(), Refusal> {
        if matches!(self, Self::Attributable { num, den }
            if num == 0 || den == 0 || num > den || gcd(num, den) != 1)
        {
            return Err(domain(
                "public ownership fraction must be positive, reduced and at most one",
            ));
        }
        Ok(())
    }
}

/// Immutable calendar target interpreted in the units declared by its metric. The adapter must authenticate
/// the original registered release and exact ownership/cost basis; a later comparative never fills it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicResolutionPolicy {
    pub issuer: String,
    pub metric: PublicMetric,
    pub period: PublicPeriod,
    pub ownership: PublicOwnership,
    pub members: BTreeMap<ObjectId, String>,
}

impl PublicResolutionPolicy {
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.issuer.is_empty()
            || self.issuer.len() > 64
            || !self
                .issuer
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
            || self.members.is_empty()
            || self.members.len() > MAX_MEMBERS
            || self.members.iter().any(|(id, subject)| {
                id.kind != PrefixKind::Forecast
                    || subject.is_empty()
                    || subject.len() > 160
                    || subject.trim() != subject
                    || subject.chars().any(char::is_control)
            })
        {
            return Err(domain(
                "invalid public-resolution issuer or forecast-to-subject mapping",
            ));
        }
        if self.members.values().collect::<BTreeSet<_>>().len() != self.members.len() {
            return Err(domain(
                "a public cohort cannot count the same physical subject more than once",
            ));
        }
        self.ownership.validate()?;
        self.period.end_ns()?;
        Ok(())
    }

    pub fn to_resolution_rule(&self) -> Result<String, Refusal> {
        self.to_resolution_rule_with_vintages(PublicVintagePolicy::OriginalOnly)
    }

    /// Original-only retains its exact v1 bytes; comparative permission is an explicit v2 commitment.
    pub fn to_resolution_rule_with_vintages(
        &self,
        vintages: PublicVintagePolicy,
    ) -> Result<String, Refusal> {
        self.validate()?;
        let (prefix, version) = match vintages {
            PublicVintagePolicy::OriginalOnly => (PUBLIC_RESOLUTION_PREFIX, 1),
            PublicVintagePolicy::RegisteredComparatives => (PUBLIC_RESOLUTION_V2_PREFIX, 2),
        };
        let mut enc = Encoder::new_frame(*b"PRP1", version);
        for value in [
            self.issuer.as_str(),
            self.metric.as_str(),
            self.metric.unit(),
            self.metric.field(self.ownership),
            &self.period.label(),
            &self.period.original_release(),
            &self.ownership.label(),
        ] {
            enc.put_str(value);
        }
        if version == 2 {
            enc.put_str(vintages.as_str());
        }
        enc.put_u32(self.members.len() as u32);
        for (id, subject) in &self.members {
            enc.put_fixed(&id.bytes);
            enc.put_str(subject);
        }
        let mut rule = String::from(prefix);
        for byte in enc.finish() {
            const HEX: &[u8] = b"0123456789abcdef";
            rule.push(char::from(HEX[usize::from(byte >> 4)]));
            rule.push(char::from(HEX[usize::from(byte & 15)]));
        }
        Ok(rule)
    }

    /// Legacy free-text rules return `None`; any malformed reserved policy is a typed refusal.
    pub fn parse_resolution_rule(rule: &str) -> Result<Option<Self>, Refusal> {
        Self::parse_resolution_rule_with_vintages(rule)
            .map(|parsed| parsed.map(|(policy, _)| policy))
    }

    /// Decode the target and its frozen vintage permission without changing legacy free-text handling.
    pub fn parse_resolution_rule_with_vintages(
        rule: &str,
    ) -> Result<Option<(Self, PublicVintagePolicy)>, Refusal> {
        if !rule.trim().to_ascii_lowercase().starts_with(RESERVED) {
            return Ok(None);
        }
        let bad = || domain("invalid or noncanonical frozen public-resolution policy");
        let (hex, version) = if let Some(hex) = rule.strip_prefix(PUBLIC_RESOLUTION_PREFIX) {
            (hex, 1)
        } else if let Some(hex) = rule.strip_prefix(PUBLIC_RESOLUTION_V2_PREFIX) {
            (hex, 2)
        } else {
            return Err(bad());
        };
        if hex.len() > 2_000_000 || hex.len() % 2 != 0 {
            return Err(bad());
        }
        let bytes = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let nibble = |b| match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    _ => None,
                };
                Ok(nibble(pair[0]).ok_or_else(bad)? * 16 + nibble(pair[1]).ok_or_else(bad)?)
            })
            .collect::<Result<Vec<_>, Refusal>>()?;
        let mut dec = Decoder::open_frame(&bytes, *b"PRP1", version, version).map_err(|_| bad())?;
        let issuer = dec.get_str(64).map_err(|_| bad())?.to_owned();
        let metric = PublicMetric::parse(dec.get_str(32).map_err(|_| bad())?)?;
        let unit = dec.get_str(32).map_err(|_| bad())?;
        let field = dec.get_str(64).map_err(|_| bad())?;
        let period = PublicPeriod::parse(dec.get_str(6).map_err(|_| bad())?)?;
        let release = dec.get_str(6).map_err(|_| bad())?;
        let ownership = PublicOwnership::parse(dec.get_str(64).map_err(|_| bad())?)?;
        if unit != metric.unit()
            || field != metric.field(ownership)
            || release != period.original_release()
        {
            return Err(bad());
        }
        let vintages = if version == 1 {
            PublicVintagePolicy::OriginalOnly
        } else {
            PublicVintagePolicy::parse(dec.get_str(32).map_err(|_| bad())?)?
        };
        let count = dec.get_u32().map_err(|_| bad())? as usize;
        if count == 0 || count > MAX_MEMBERS {
            return Err(bad());
        }
        let mut members = BTreeMap::new();
        for _ in 0..count {
            let id = ObjectId::from_raw(PrefixKind::Forecast, dec.get_fixed().map_err(|_| bad())?);
            let subject = dec.get_str(160).map_err(|_| bad())?.to_owned();
            if members.insert(id, subject).is_some() {
                return Err(bad());
            }
        }
        dec.finish().map_err(|_| bad())?;
        let policy = Self {
            issuer,
            metric,
            period,
            ownership,
            members,
        };
        if policy.to_resolution_rule_with_vintages(vintages)? != rule {
            return Err(bad());
        }
        Ok(Some((policy, vintages)))
    }

    pub fn validate_cohort(&self, spec: &CohortSpec) -> Result<(), Refusal> {
        self.validate()?;
        let mut members = spec.members.clone();
        members.sort();
        if members != self.members.keys().copied().collect::<Vec<_>>()
            || spec.eligible_sources != [self.issuer.clone()]
        {
            return Err(domain(
                "public policy must bind exactly the cohort members and its sole issuer source",
            ));
        }
        Ok(())
    }

    pub fn validate_forecast(&self, forecast: &FrozenForecast) -> Result<(), Refusal> {
        if !self.members.contains_key(&forecast.forecast_id) {
            return Err(domain(
                "forecast is absent from its public-resolution policy",
            ));
        }
        if forecast.freeze_time_ns > self.period.end_ns()? {
            return Err(late(
                "public forecast deadline cannot follow the end of its frozen target period",
            ));
        }
        if matches!(forecast.payload, ForecastPayload::Bernoulli { .. }) {
            return Err(domain(
                "a public production or cost cell requires a continuous forecast",
            ));
        }
        Ok(())
    }

    pub fn validate_shared_targets(&self, other: &Self) -> Result<(), Refusal> {
        for (id, subject) in &self.members {
            if let Some(prior) = other.members.get(id)
                && (self.issuer != other.issuer
                    || self.metric != other.metric
                    || self.period != other.period
                    || self.ownership != other.ownership
                    || subject != prior)
            {
                return Err(frozen(
                    "a forecast already has a different frozen public-resolution target",
                ));
            }
        }
        Ok(())
    }
}

const fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{AssessmentPolicy, ForecastLedger};
    use fmine_core::codec::crc32c;

    fn policy(ids: &[ObjectId]) -> PublicResolutionPolicy {
        PublicResolutionPolicy {
            issuer: "barrick".into(),
            metric: PublicMetric::ProductionKoz,
            period: PublicPeriod::Year(1970),
            ownership: PublicOwnership::Full,
            members: ids
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, format!("site_{i}")))
                .collect(),
        }
    }

    fn forecasts(deadline: i64) -> (ForecastLedger, Vec<ObjectId>) {
        let mut ledger = ForecastLedger::new();
        let ids = (0..2)
            .map(|i| {
                ledger
                    .register_payload(
                        format!("model_{i}"),
                        1,
                        deadline,
                        ForecastPayload::Interval {
                            lo: 100.0,
                            hi: 120.0,
                            nominal: None,
                        },
                    )
                    .unwrap()
            })
            .collect();
        (ledger, ids)
    }

    fn spec(policy: &PublicResolutionPolicy, name: &str, time: i64) -> CohortSpec {
        CohortSpec {
            name: name.into(),
            frozen_at_ns: time,
            members: policy.members.keys().copied().collect(),
            resolution_rule: policy.to_resolution_rule().unwrap(),
            eligible_sources: vec![policy.issuer.clone()],
            policy: AssessmentPolicy::default(),
        }
    }

    #[test]
    fn vintage_permission_keeps_the_v1_known_answer_and_changes_the_v2_commitment() {
        let policy = policy(&[ObjectId::from_raw(PrefixKind::Forecast, [1; 16])]);
        // Independent frame-specification known answer: length-prefixed UTF-8 fields, one raw id,
        // and CRC32C. No candidate serializer produced this literal.
        let original = concat!(
            "fmine.public-resolution.v1:",
            "464d4e45505250310100070000006261727269636b0e00000070726f64756374696f6e5f6b6f7a",
            "030000006b6f7a1700000073756d6d6172793a61755f70726f645f3130305f6b6f7a0600000031",
            "3937304659060000003139373051340400000031303025010000000101010101010101010101",
            "010101010106000000736974655f305cd3eb77",
        );
        assert_eq!(policy.to_resolution_rule().unwrap(), original);
        assert_eq!(
            policy
                .to_resolution_rule_with_vintages(PublicVintagePolicy::OriginalOnly)
                .unwrap(),
            original
        );
        assert_eq!(
            PublicResolutionPolicy::parse_resolution_rule_with_vintages(original).unwrap(),
            Some((policy.clone(), PublicVintagePolicy::OriginalOnly))
        );
        let comparative = policy
            .to_resolution_rule_with_vintages(PublicVintagePolicy::RegisteredComparatives)
            .unwrap();
        assert!(comparative.starts_with(PUBLIC_RESOLUTION_V2_PREFIX));
        assert_ne!(comparative, original);
        assert_eq!(
            PublicResolutionPolicy::parse_resolution_rule_with_vintages(&comparative).unwrap(),
            Some((policy.clone(), PublicVintagePolicy::RegisteredComparatives))
        );
        assert_eq!(
            PublicResolutionPolicy::parse_resolution_rule(&comparative).unwrap(),
            Some(policy)
        );
        assert_eq!(
            PublicResolutionPolicy::parse_resolution_rule_with_vintages("legacy rule").unwrap(),
            None
        );
        assert!(PublicVintagePolicy::parse("latest").is_err());
    }

    #[test]
    fn comparative_permission_never_replaces_an_original_or_accepts_an_earlier_release() {
        for target in [
            PublicPeriod::Year(2026),
            PublicPeriod::Quarter {
                year: 2026,
                quarter: 1,
            },
        ] {
            let original = target.original_release();
            for mode in [
                PublicVintagePolicy::OriginalOnly,
                PublicVintagePolicy::RegisteredComparatives,
            ] {
                assert!(mode.validate_release(target, &original, false).is_ok());
                assert!(mode.validate_release(target, &original, true).is_err());
                assert!(mode.validate_release(target, "2027Q1", false).is_err());
                assert_eq!(
                    mode.validate_release(target, "2027Q1", true).is_ok(),
                    mode == PublicVintagePolicy::RegisteredComparatives
                );
                for bad in ["2025Q4", "2027FY", "2027Q0", "2027Q5", "2027Q1 ", "9999Q1"] {
                    assert!(mode.validate_release(target, bad, true).is_err(), "{bad}");
                }
            }
        }
        assert!(
            PublicVintagePolicy::RegisteredComparatives
                .validate_release(
                    PublicPeriod::Quarter {
                        year: 2026,
                        quarter: 1
                    },
                    "2026Q2",
                    true
                )
                .is_ok()
        );
    }

    #[test]
    fn changing_a_shared_forecasts_vintage_permission_refuses_without_advancing_the_ledger() {
        for initial in [
            PublicVintagePolicy::OriginalOnly,
            PublicVintagePolicy::RegisteredComparatives,
        ] {
            let other = if initial == PublicVintagePolicy::OriginalOnly {
                PublicVintagePolicy::RegisteredComparatives
            } else {
                PublicVintagePolicy::OriginalOnly
            };
            let (mut ledger, ids) = forecasts(PublicPeriod::Year(1970).end_ns().unwrap());
            let policy = policy(&ids[..1]);
            let mut first = spec(&policy, "first", 2);
            first.resolution_rule = policy.to_resolution_rule_with_vintages(initial).unwrap();
            let first_digest = ledger.freeze_cohort(first.clone()).unwrap();
            let mut conflicting = spec(&policy, "second", 200);
            conflicting.resolution_rule = policy.to_resolution_rule_with_vintages(other).unwrap();
            assert_eq!(
                ledger.freeze_cohort(conflicting).unwrap_err().code,
                "FMINE-REF-507"
            );
            let mut compatible = first.clone();
            compatible.name = "second".into();
            compatible.frozen_at_ns = 3;
            assert!(ledger.freeze_cohort(compatible).is_ok());
            assert_eq!(
                ledger.cohort(first_digest).unwrap().spec.resolution_rule,
                first.resolution_rule
            );

            // A different forecast may use a different permission, even for the same physical subject.
            let mut disjoint = policy.clone();
            disjoint.members = BTreeMap::from([(ids[1], "site_0".into())]);
            let mut third = spec(&disjoint, "third", 4);
            third.resolution_rule = disjoint.to_resolution_rule_with_vintages(other).unwrap();
            assert!(ledger.freeze_cohort(third).is_ok());
        }
        let (mut original_ledger, ids) = forecasts(PublicPeriod::Year(1970).end_ns().unwrap());
        let (mut comparative_ledger, _) = forecasts(PublicPeriod::Year(1970).end_ns().unwrap());
        let policy = policy(&ids);
        let original = spec(&policy, "same", 2);
        let mut comparative = original.clone();
        comparative.resolution_rule = policy
            .to_resolution_rule_with_vintages(PublicVintagePolicy::RegisteredComparatives)
            .unwrap();
        assert_ne!(
            original_ledger.freeze_cohort(original).unwrap(),
            comparative_ledger.freeze_cohort(comparative).unwrap()
        );
    }

    #[test]
    fn prefix_frame_version_and_explicit_vintage_mode_must_be_canonical() {
        let policy = policy(&[ObjectId::from_raw(PrefixKind::Forecast, [1; 16])]);
        let original = policy.to_resolution_rule().unwrap();
        let comparative = policy
            .to_resolution_rule_with_vintages(PublicVintagePolicy::RegisteredComparatives)
            .unwrap();
        for forged in [
            comparative.replacen(PUBLIC_RESOLUTION_V2_PREFIX, PUBLIC_RESOLUTION_PREFIX, 1),
            original.replacen(PUBLIC_RESOLUTION_PREFIX, PUBLIC_RESOLUTION_V2_PREFIX, 1),
            comparative.replacen("v2:", "v3:", 1),
            format!("{comparative}00"),
        ] {
            assert!(PublicResolutionPolicy::parse_resolution_rule_with_vintages(&forged).is_err());
        }
        for mode in ["original_only", "unknown"] {
            let mut enc = Encoder::new_frame(*b"PRP1", 2);
            for field in [
                "barrick",
                "production_koz",
                "koz",
                "summary:au_prod_100_koz",
                "1970FY",
                "1970Q4",
                "100%",
                mode,
            ] {
                enc.put_str(field);
            }
            enc.put_u32(1);
            enc.put_fixed(&[1; 16]);
            enc.put_str("site_0");
            let forged = format!(
                "{PUBLIC_RESOLUTION_V2_PREFIX}{}",
                enc.finish()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            assert!(PublicResolutionPolicy::parse_resolution_rule_with_vintages(&forged).is_err());
        }
    }

    #[test]
    fn calendar_end_known_answers_and_original_releases() {
        assert_eq!(
            PublicPeriod::parse("1970Q1").unwrap().end_ns().unwrap(),
            7_775_999_999_999_999
        );
        assert_eq!(
            PublicPeriod::parse("1970FY").unwrap().end_ns().unwrap(),
            31_535_999_999_999_999
        );
        assert_eq!(
            PublicPeriod::parse("2000Q1").unwrap().end_ns().unwrap(),
            954_547_199_999_999_999
        );
        assert_eq!(PublicPeriod::Year(2026).original_release(), "2026Q4");
        assert_eq!(
            PublicPeriod::parse("2026Q2").unwrap().original_release(),
            "2026Q2"
        );
        for invalid in ["2026Q0", "2026Q5", "2026Y", "2026Q１", "9999FY", "0000FY"] {
            assert!(PublicPeriod::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn canonical_roundtrip_binds_units_fields_and_reduced_ownership() {
        let ids = [
            ObjectId::from_raw(PrefixKind::Forecast, [2; 16]),
            ObjectId::from_raw(PrefixKind::Forecast, [1; 16]),
        ];
        let mut policy = policy(&ids);
        policy.ownership = PublicOwnership::parse("attributable:8/10").unwrap();
        assert_eq!(
            policy.ownership,
            PublicOwnership::Attributable { num: 4, den: 5 }
        );
        assert_eq!(
            policy.metric.field(policy.ownership),
            "summary:au_prod_attr_koz"
        );
        assert_eq!(PublicMetric::AiscUsdPerOz.unit(), "usd_per_oz");
        let rule = policy.to_resolution_rule().unwrap();
        assert_eq!(
            PublicResolutionPolicy::parse_resolution_rule(&rule).unwrap(),
            Some(policy.clone())
        );
        let mut reordered = policy.clone();
        reordered.members = policy
            .members
            .iter()
            .rev()
            .map(|(id, site)| (*id, site.clone()))
            .collect();
        assert_eq!(reordered.to_resolution_rule().unwrap(), rule);
        reordered.members.insert(ids[0], "another_site".into());
        assert_ne!(reordered.to_resolution_rule().unwrap(), rule);
        policy.ownership = PublicOwnership::Attributable { num: 8, den: 10 };
        assert!(policy.to_resolution_rule().is_err());
        policy.ownership = PublicOwnership::Attributable { num: 1, den: 1 };
        let attributable = policy.to_resolution_rule().unwrap();
        policy.ownership = PublicOwnership::Full;
        assert_ne!(attributable, policy.to_resolution_rule().unwrap());
        for invalid in [
            "",
            "attributable:0/1",
            "attributable:1/0",
            "attributable:2/1",
            "unknown",
        ] {
            assert!(PublicOwnership::parse(invalid).is_err());
        }
        policy.members.insert(
            ObjectId::from_raw(PrefixKind::Claim, [3; 16]),
            "site_c".into(),
        );
        assert!(policy.to_resolution_rule().is_err());
    }

    #[test]
    fn reserved_syntax_crc_and_forged_unit_refuse_without_affecting_legacy() {
        let id = ObjectId::from_raw(PrefixKind::Forecast, [1; 16]);
        let rule = policy(&[id]).to_resolution_rule().unwrap();
        for bad in [
            PUBLIC_RESOLUTION_PREFIX.into(),
            "fmine.public-resolution.v2:00".into(),
            format!(" {rule}"),
            format!("{rule}00"),
            rule.to_uppercase(),
        ] {
            assert!(
                PublicResolutionPolicy::parse_resolution_rule(&bad).is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            PublicResolutionPolicy::parse_resolution_rule("original published value").unwrap(),
            None
        );
        let hex = rule.strip_prefix(PUBLIC_RESOLUTION_PREFIX).unwrap();
        let mut bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect();
        let offset = bytes.windows(3).position(|p| p == b"koz").unwrap();
        // First occurrence is the metric suffix: preserve the metric, forge only its explicit unit.
        let unit = bytes[offset + 3..]
            .windows(3)
            .position(|p| p == b"koz")
            .unwrap()
            + offset
            + 3;
        bytes[unit..unit + 3].copy_from_slice(b"USD");
        let tail = bytes.len() - 4;
        let checksum = crc32c(&bytes[..tail]);
        bytes[tail..].copy_from_slice(&checksum.to_le_bytes());
        let forged = format!(
            "{PUBLIC_RESOLUTION_PREFIX}{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        assert!(PublicResolutionPolicy::parse_resolution_rule(&forged).is_err());
        bytes[0] ^= 1;
        let corrupt = format!(
            "{PUBLIC_RESOLUTION_PREFIX}{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        assert!(PublicResolutionPolicy::parse_resolution_rule(&corrupt).is_err());
    }

    #[test]
    fn core_freeze_requires_exact_members_and_singleton_issuer_atomically() {
        let (mut ledger, ids) = forecasts(PublicPeriod::Year(1970).end_ns().unwrap());
        let policy = policy(&ids);
        let valid = spec(&policy, "public", 2);
        let mut missing = valid.clone();
        missing.members.pop();
        let mut extra_source = valid.clone();
        extra_source.eligible_sources.push("another_issuer".into());
        let mut wrong_source = valid.clone();
        wrong_source.eligible_sources = vec!["another_issuer".into()];
        for invalid in [missing, extra_source, wrong_source] {
            assert_eq!(
                ledger.freeze_cohort(invalid).unwrap_err().code,
                "FMINE-REF-109"
            );
        }
        let digest = ledger.freeze_cohort(valid).unwrap();
        assert_eq!(ledger.cohort(digest).unwrap().spec.members.len(), 2);
    }

    #[test]
    fn repeated_physical_target_refuses_atomically_but_separate_models_keep_one_outcome_each() {
        use crate::ledger::{OutcomeCitation, OutcomeKind, OutcomeSelector};

        let end = PublicPeriod::Year(1970).end_ns().unwrap();
        let (mut ledger, ids) = forecasts(end);
        assert_ne!(ids[0], ids[1]);
        let mut duplicate = policy(&ids);
        duplicate
            .members
            .values_mut()
            .for_each(|subject| *subject = "site_a".into());
        assert_eq!(
            duplicate.to_resolution_rule().unwrap_err().code,
            "FMINE-REF-109"
        );
        // A correctly framed client rule must not bypass the same guard at the ledger boundary.
        let mut enc = Encoder::new_frame(*b"PRP1", 1);
        for field in [
            "barrick",
            "production_koz",
            "koz",
            "summary:au_prod_100_koz",
            "1970FY",
            "1970Q4",
            "100%",
        ] {
            enc.put_str(field);
        }
        enc.put_u32(2);
        for id in duplicate.members.keys() {
            enc.put_fixed(&id.bytes);
            enc.put_str("site_a");
        }
        let mut invalid = spec(&policy(&ids), "model_a", 200);
        invalid.resolution_rule = format!(
            "{PUBLIC_RESOLUTION_PREFIX}{}",
            enc.finish()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        assert_eq!(
            ledger.freeze_cohort(invalid).unwrap_err().code,
            "FMINE-REF-109"
        );

        // Reuse the refused name and earlier record times: neither name nor watermark was advanced.
        let mut single = policy(&ids[..1]);
        single.members.insert(ids[0], "site_a".into());
        let first = ledger.freeze_cohort(spec(&single, "model_a", 2)).unwrap();
        single.members = BTreeMap::from([(ids[1], "site_a".into())]);
        let second = ledger.freeze_cohort(spec(&single, "model_b", 3)).unwrap();
        for id in ids {
            ledger
                .append_outcome(
                    id,
                    end + 1,
                    end + 1,
                    OutcomeKind::Published {
                        value: 110.0,
                        citation: OutcomeCitation {
                            source: "barrick".into(),
                            vintage: "invented-original".into(),
                            locator: "invented:site_a".into(),
                            sha256: [1; 32],
                        },
                    },
                )
                .unwrap();
        }
        for cohort in [first, second] {
            let report = ledger
                .assess(cohort, end + 2, OutcomeSelector::FirstPublished)
                .unwrap();
            assert_eq!(
                (report.summary.n_members, report.summary.n_resolved),
                (1, 1)
            );
            assert_eq!(report.summary.flag, crate::CalibrationFlag::Descriptive);
        }
    }

    #[test]
    fn deadline_at_or_before_period_end_is_allowed_but_late_targets_refuse() {
        let end = PublicPeriod::Year(1970).end_ns().unwrap();
        for deadline in [end - 1, end, end + 1] {
            let (mut ledger, ids) = forecasts(deadline);
            let result = ledger.freeze_cohort(spec(&policy(&ids), "public", 2));
            if deadline <= end {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.unwrap_err().code, "FMINE-REF-206");
                let mut legacy = spec(&policy(&ids), "public", 2);
                legacy.resolution_rule = "legacy free text".into();
                assert!(ledger.freeze_cohort(legacy).is_ok());
            }
        }
        let mut ledger = ForecastLedger::new();
        let id = ledger
            .register_payload(
                "late",
                end + 1,
                end + 4,
                ForecastPayload::Interval {
                    lo: 100.0,
                    hi: 120.0,
                    nominal: None,
                },
            )
            .unwrap();
        assert_eq!(
            ledger
                .freeze_cohort(spec(&policy(&[id]), "late", end + 2))
                .unwrap_err()
                .code,
            "FMINE-REF-206"
        );
    }

    #[test]
    fn a_shared_forecast_cannot_acquire_a_conflicting_public_target() {
        let (mut ledger, ids) = forecasts(PublicPeriod::Year(1970).end_ns().unwrap());
        let first = policy(&ids[..1]);
        let digest = ledger.freeze_cohort(spec(&first, "first", 2)).unwrap();
        for mutation in 0..5 {
            let mut changed = first.clone();
            match mutation {
                0 => changed.issuer = "another_issuer".into(),
                1 => changed.metric = PublicMetric::AiscUsdPerOz,
                2 => changed.period = PublicPeriod::Year(1971),
                3 => changed.ownership = PublicOwnership::Attributable { num: 1, den: 1 },
                _ => {
                    changed.members.insert(ids[0], "another_site".into());
                }
            }
            assert_eq!(
                ledger
                    .freeze_cohort(spec(&changed, "second", 3))
                    .unwrap_err()
                    .code,
                "FMINE-REF-507"
            );
        }
        let full = policy(&ids);
        assert!(ledger.freeze_cohort(spec(&full, "second", 3)).is_ok());
        assert_eq!(
            ledger.cohort(digest).unwrap().spec.resolution_rule,
            first.to_resolution_rule().unwrap()
        );
        let mut legacy = spec(&full, "legacy", 4);
        legacy.resolution_rule = "independently frozen legacy rule".into();
        assert!(ledger.freeze_cohort(legacy).is_ok());
    }

    #[test]
    fn binary_events_cannot_be_reinterpreted_as_continuous_public_cells() {
        let mut ledger = ForecastLedger::new();
        let id = ledger
            .register_payload(
                "event",
                1,
                100,
                ForecastPayload::Bernoulli { probability: 0.5 },
            )
            .unwrap();
        assert_eq!(
            ledger
                .freeze_cohort(spec(&policy(&[id]), "event", 2))
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );
    }
}
