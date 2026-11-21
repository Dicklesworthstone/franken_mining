//! Determinism: BitwisePortable
//!
//! Fail-closed data residency registry (`registries/residency.toml`, schema `fmine.residency/1`;
//! plan §31.2, `BD-094`, W3 `E501–E549`).
//!
//! Any `(country, data_class)` pair not present in the registry defaults to [`ResidencyRule::InCountryOnly`].
//! A registry that declares any `default_rule` other than `"in_country_only"` is refused on parse.
//! Rows declaring a `requires` legal instrument remain held at `in_country_only` until that instrument is
//! explicitly registered in the evaluation context (plan §31.2: Pakistan `geoscience`, etc.).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::AuthzError;

/// Expected schema version for `registries/residency.toml`.
pub const RESIDENCY_SCHEMA: &str = "fmine.residency/1";

/// Six statutory data classes tracked in `registries/residency.toml` (§31.2, W3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataClass {
    Personal,
    Geoscience,
    Production,
    Financial,
    StateSecret,
    Aggregate,
}

impl DataClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Geoscience => "geoscience",
            Self::Production => "production",
            Self::Financial => "financial",
            Self::StateSecret => "state_secret",
            Self::Aggregate => "aggregate",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "personal" => Some(Self::Personal),
            "geoscience" => Some(Self::Geoscience),
            "production" => Some(Self::Production),
            "financial" => Some(Self::Financial),
            "state_secret" => Some(Self::StateSecret),
            "aggregate" => Some(Self::Aggregate),
            _ => None,
        }
    }
}

/// Three-valued fail-closed residency contract (§31.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResidencyRule {
    InCountryOnly,
    AggregatesExportable,
    Exportable,
}

impl ResidencyRule {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InCountryOnly => "in_country_only",
            Self::AggregatesExportable => "aggregates_exportable",
            Self::Exportable => "exportable",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "in_country_only" => Some(Self::InCountryOnly),
            "aggregates_exportable" => Some(Self::AggregatesExportable),
            "exportable" => Some(Self::Exportable),
            _ => None,
        }
    }
}

/// Evidence label carried by each residency row (§31.2, W3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidencyStatus {
    Fact,
    FactUnverified,
    Inference,
}

impl ResidencyStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "FACT" => Some(Self::Fact),
            "FACT[U]" => Some(Self::FactUnverified),
            "INFERENCE" => Some(Self::Inference),
            _ => None,
        }
    }
}

/// One `(country, data_class)` policy row from `registries/residency.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidencyRow {
    pub country: String,
    pub data_class: DataClass,
    pub rule: ResidencyRule,
    pub basis: String,
    pub status: ResidencyStatus,
    /// Confidence scaled in basis points (`0..=10_000`, e.g. `0.60` -> `6000`) for exact portability.
    pub confidence_bps: u16,
    pub review_by: String,
    pub requires: Option<String>,
    pub allow_destinations: Vec<String>,
    pub deny_destinations: Vec<String>,
    pub evidence: Vec<String>,
}

/// Outcome of evaluating a cross-border data transfer against [`ResidencyRegistry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidencyDecision {
    /// Transfer is permitted under the active `residency_epoch`.
    Allowed {
        residency_epoch: u64,
        effective_rule: ResidencyRule,
    },
    /// Transfer is refused before bytes move (`FMINE-REF-303`, exit 7).
    Refused(AuthzError),
}

/// Parsed, fail-closed data residency registry (`BD-094`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidencyRegistry {
    pub schema: String,
    pub residency_epoch: u64,
    pub as_of: String,
    pub default_rule: ResidencyRule,
    rows: BTreeMap<(String, DataClass), ResidencyRow>,
}

impl ResidencyRegistry {
    /// Loads and validates `<workspace_root>/registries/residency.toml`.
    pub fn load_from_root(workspace_root: &Path) -> Result<Self, AuthzError> {
        let path = workspace_root.join("registries/residency.toml");
        let text = fs::read_to_string(&path).map_err(|e| AuthzError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        Self::parse(&text)
    }

    /// Parses and validates `registries/residency.toml`, refusing unknown keys, duplicates,
    /// malformed values, or any fail-open `default_rule`.
    pub fn parse(text: &str) -> Result<Self, AuthzError> {
        let mut schema: Option<String> = None;
        let mut residency_epoch: Option<u64> = None;
        let mut as_of: Option<String> = None;
        let mut default_rule: Option<ResidencyRule> = None;
        let mut raw_rows: Vec<(usize, BTreeMap<String, RawVal>)> = Vec::new();
        let mut in_row = false;

        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let stripped = strip_comment(raw_line).trim();
            if stripped.is_empty() {
                continue;
            }
            if stripped == "[[row]]" {
                in_row = true;
                raw_rows.push((line_no, BTreeMap::new()));
                continue;
            }
            if stripped.starts_with('[') {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line: line_no,
                    detail: format!("unsupported table header {stripped:?}"),
                });
            }
            let Some((k, v)) = stripped.split_once('=') else {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line: line_no,
                    detail: "expected `key = value`".to_string(),
                });
            };
            let key = k.trim();
            let val = parse_value(v.trim(), line_no)?;
            if !in_row {
                match key {
                    "schema" => {
                        let s = val.into_str(line_no, "schema")?;
                        if schema.replace(s).is_some() {
                            return Err(dup_key(line_no, "schema"));
                        }
                    }
                    "residency_epoch" => {
                        let n = val.into_u64(line_no, "residency_epoch")?;
                        if residency_epoch.replace(n).is_some() {
                            return Err(dup_key(line_no, "residency_epoch"));
                        }
                    }
                    "as_of" => {
                        let s = val.into_str(line_no, "as_of")?;
                        validate_iso_date(&s, line_no, "as_of")?;
                        if as_of.replace(s).is_some() {
                            return Err(dup_key(line_no, "as_of"));
                        }
                    }
                    "default_rule" => {
                        let s = val.into_str(line_no, "default_rule")?;
                        let rule = ResidencyRule::parse(&s).ok_or_else(|| {
                            AuthzError::ResidencyRegistryInvalid {
                                line: line_no,
                                detail: format!("unknown default_rule {s:?}"),
                            }
                        })?;
                        if default_rule.replace(rule).is_some() {
                            return Err(dup_key(line_no, "default_rule"));
                        }
                    }
                    other => {
                        return Err(AuthzError::ResidencyRegistryInvalid {
                            line: line_no,
                            detail: format!("unknown top-level key {other:?}"),
                        });
                    }
                }
            } else if let Some((_, fields)) = raw_rows.last_mut()
                && fields.insert(key.to_string(), val).is_some()
            {
                return Err(dup_key(line_no, key));
            }
        }

        let schema = schema.ok_or_else(|| AuthzError::ResidencyRegistryInvalid {
            line: 1,
            detail: "missing `schema`".to_string(),
        })?;
        if schema != RESIDENCY_SCHEMA {
            return Err(AuthzError::ResidencyRegistryInvalid {
                line: 1,
                detail: format!("expected schema {RESIDENCY_SCHEMA:?}, found {schema:?}"),
            });
        }
        let residency_epoch =
            residency_epoch.ok_or_else(|| AuthzError::ResidencyRegistryInvalid {
                line: 1,
                detail: "missing `residency_epoch`".to_string(),
            })?;
        if residency_epoch == 0 {
            return Err(AuthzError::ResidencyRegistryInvalid {
                line: 1,
                detail: "`residency_epoch` must be >= 1".to_string(),
            });
        }
        let as_of = as_of.ok_or_else(|| AuthzError::ResidencyRegistryInvalid {
            line: 1,
            detail: "missing `as_of`".to_string(),
        })?;
        let default_rule = default_rule.ok_or_else(|| AuthzError::ResidencyRegistryInvalid {
            line: 1,
            detail: "missing `default_rule`".to_string(),
        })?;
        if default_rule != ResidencyRule::InCountryOnly {
            return Err(AuthzError::ResidencyRegistryInvalid {
                line: 1,
                detail: format!(
                    "fail-closed invariant violated: default_rule must be \"in_country_only\", found \"{}\"",
                    default_rule.as_str()
                ),
            });
        }

        let mut rows = BTreeMap::new();
        for (row_line, mut fields) in raw_rows {
            let country =
                take_required(&mut fields, "country", row_line)?.into_str(row_line, "country")?;
            validate_iso_country(&country, row_line, "country")?;
            let dc_str = take_required(&mut fields, "data_class", row_line)?
                .into_str(row_line, "data_class")?;
            let data_class =
                DataClass::parse(&dc_str).ok_or_else(|| AuthzError::ResidencyRegistryInvalid {
                    line: row_line,
                    detail: format!("unknown data_class {dc_str:?}"),
                })?;
            let rule_str =
                take_required(&mut fields, "rule", row_line)?.into_str(row_line, "rule")?;
            let rule = ResidencyRule::parse(&rule_str).ok_or_else(|| {
                AuthzError::ResidencyRegistryInvalid {
                    line: row_line,
                    detail: format!("unknown rule {rule_str:?}"),
                }
            })?;
            let basis =
                take_required(&mut fields, "basis", row_line)?.into_str(row_line, "basis")?;
            if basis.trim().is_empty() {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line: row_line,
                    detail: "`basis` must not be empty".to_string(),
                });
            }
            let status_str =
                take_required(&mut fields, "status", row_line)?.into_str(row_line, "status")?;
            let status = ResidencyStatus::parse(&status_str).ok_or_else(|| {
                AuthzError::ResidencyRegistryInvalid {
                    line: row_line,
                    detail: format!("unknown status {status_str:?}"),
                }
            })?;
            let confidence_bps = take_required(&mut fields, "confidence", row_line)?
                .into_confidence_bps(row_line)?;
            let review_by = take_required(&mut fields, "review_by", row_line)?
                .into_str(row_line, "review_by")?;
            validate_iso_date(&review_by, row_line, "review_by")?;
            let evidence = take_required(&mut fields, "evidence", row_line)?
                .into_str_list(row_line, "evidence")?;
            let requires = match fields.remove("requires") {
                Some(v) => Some(v.into_str(row_line, "requires")?),
                None => None,
            };
            let allow_destinations = match fields.remove("allow_destinations") {
                Some(v) => {
                    let list = v.into_str_list(row_line, "allow_destinations")?;
                    for c in &list {
                        validate_iso_country(c, row_line, "allow_destinations")?;
                    }
                    list
                }
                None => Vec::new(),
            };
            let deny_destinations = match fields.remove("deny_destinations") {
                Some(v) => {
                    let list = v.into_str_list(row_line, "deny_destinations")?;
                    for c in &list {
                        validate_iso_country(c, row_line, "deny_destinations")?;
                    }
                    list
                }
                None => Vec::new(),
            };
            if let Some((unknown, _)) = fields.into_iter().next() {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line: row_line,
                    detail: format!("unknown row key {unknown:?}"),
                });
            }
            let row = ResidencyRow {
                country: country.clone(),
                data_class,
                rule,
                basis,
                status,
                confidence_bps,
                review_by,
                requires,
                allow_destinations,
                deny_destinations,
                evidence,
            };
            if rows.insert((country.clone(), data_class), row).is_some() {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line: row_line,
                    detail: format!(
                        "duplicate residency row for ({country}, {})",
                        data_class.as_str()
                    ),
                });
            }
        }

        Ok(Self {
            schema,
            residency_epoch,
            as_of,
            default_rule,
            rows,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn row(&self, country: &str, data_class: DataClass) -> Option<&ResidencyRow> {
        self.rows.get(&(country.to_string(), data_class))
    }

    /// Returns the fail-closed effective rule for `(country, data_class)` given the set of
    /// registered legal instruments (`requires` satisfiers).
    ///
    /// Unknown pairs ALWAYS return [`ResidencyRule::InCountryOnly`] (`BD-094`).
    /// Rows with an unsatisfied `requires` instrument are held at [`ResidencyRule::InCountryOnly`]
    /// (plan §31.2: e.g. Pakistan `geoscience` awaits counsel's reading of BMR r.71(3)(i)).
    #[must_use]
    pub fn effective_rule(
        &self,
        country: &str,
        data_class: DataClass,
        registered_instruments: &BTreeSet<String>,
    ) -> ResidencyRule {
        let Some(row) = self.row(country, data_class) else {
            return self.default_rule;
        };
        if let Some(req) = &row.requires
            && row.rule != ResidencyRule::InCountryOnly
            && !registered_instruments.contains(req)
        {
            return ResidencyRule::InCountryOnly;
        }
        row.rule
    }

    /// Evaluates whether bytes of `data_class` originating in `origin_country` may transfer to
    /// `destination_country` BEFORE any bytes move (`FMINE-REF-303`, exit 7).
    #[must_use]
    pub fn evaluate_transfer(
        &self,
        origin_country: &str,
        data_class: DataClass,
        destination_country: &str,
        is_aggregate: bool,
        registered_instruments: &BTreeSet<String>,
    ) -> ResidencyDecision {
        if origin_country == destination_country {
            return ResidencyDecision::Allowed {
                residency_epoch: self.residency_epoch,
                effective_rule: self.effective_rule(
                    origin_country,
                    data_class,
                    registered_instruments,
                ),
            };
        }

        let Some(row) = self.row(origin_country, data_class) else {
            return ResidencyDecision::Refused(AuthzError::ResidencyRefused {
                origin: origin_country.to_string(),
                destination: destination_country.to_string(),
                data_class: data_class.as_str().to_string(),
                detail: "unregistered (country, data_class) pair defaults to in_country_only"
                    .to_string(),
            });
        };

        if row
            .deny_destinations
            .iter()
            .any(|d| d == destination_country)
        {
            return ResidencyDecision::Refused(AuthzError::ResidencyRefused {
                origin: origin_country.to_string(),
                destination: destination_country.to_string(),
                data_class: data_class.as_str().to_string(),
                detail: format!("destination {destination_country} is explicitly denied"),
            });
        }

        let effective = self.effective_rule(origin_country, data_class, registered_instruments);
        match effective {
            ResidencyRule::Exportable => ResidencyDecision::Allowed {
                residency_epoch: self.residency_epoch,
                effective_rule: effective,
            },
            ResidencyRule::AggregatesExportable => {
                if is_aggregate {
                    ResidencyDecision::Allowed {
                        residency_epoch: self.residency_epoch,
                        effective_rule: effective,
                    }
                } else {
                    ResidencyDecision::Refused(AuthzError::ResidencyRefused {
                        origin: origin_country.to_string(),
                        destination: destination_country.to_string(),
                        data_class: data_class.as_str().to_string(),
                        detail:
                            "only anonymised aggregates are exportable; raw rows stay in-country"
                                .to_string(),
                    })
                }
            }
            ResidencyRule::InCountryOnly => {
                let explicitly_allowed = row
                    .allow_destinations
                    .iter()
                    .any(|d| d == destination_country)
                    && row
                        .requires
                        .as_ref()
                        .is_none_or(|req| registered_instruments.contains(req));
                if explicitly_allowed {
                    ResidencyDecision::Allowed {
                        residency_epoch: self.residency_epoch,
                        effective_rule: ResidencyRule::Exportable,
                    }
                } else {
                    let detail = match &row.requires {
                        Some(req) => {
                            format!("held in_country_only pending registered instrument: {req}")
                        }
                        None => "in_country_only forbids cross-border transfer".to_string(),
                    };
                    ResidencyDecision::Refused(AuthzError::ResidencyRefused {
                        origin: origin_country.to_string(),
                        destination: destination_country.to_string(),
                        data_class: data_class.as_str().to_string(),
                        detail,
                    })
                }
            }
        }
    }

    /// Counts rows by `(in_country_only, aggregates_exportable, exportable)`.
    #[must_use]
    pub fn rule_counts(&self) -> (usize, usize, usize) {
        let mut inc = 0;
        let mut agg = 0;
        let mut exp = 0;
        for row in self.rows.values() {
            match row.rule {
                ResidencyRule::InCountryOnly => inc += 1,
                ResidencyRule::AggregatesExportable => agg += 1,
                ResidencyRule::Exportable => exp += 1,
            }
        }
        (inc, agg, exp)
    }
}

#[derive(Debug, Clone)]
enum RawVal {
    Str(String),
    Number(String),
    StrList(Vec<String>),
}

impl RawVal {
    fn into_str(self, line: usize, key: &str) -> Result<String, AuthzError> {
        match self {
            Self::Str(s) => Ok(s),
            _ => Err(AuthzError::ResidencyRegistryInvalid {
                line,
                detail: format!("field `{key}` must be a quoted string"),
            }),
        }
    }

    fn into_u64(self, line: usize, key: &str) -> Result<u64, AuthzError> {
        match self {
            Self::Number(n) => n
                .parse::<u64>()
                .map_err(|_| AuthzError::ResidencyRegistryInvalid {
                    line,
                    detail: format!("field `{key}` must be an unsigned integer, found {n:?}"),
                }),
            _ => Err(AuthzError::ResidencyRegistryInvalid {
                line,
                detail: format!("field `{key}` must be an unsigned integer"),
            }),
        }
    }

    fn into_confidence_bps(self, line: usize) -> Result<u16, AuthzError> {
        let raw = match self {
            Self::Number(s) => s,
            _ => {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line,
                    detail: "field `confidence` must be a decimal in [0.0, 1.0]".to_string(),
                });
            }
        };
        let (whole_s, frac_s) = match raw.split_once('.') {
            Some((w, f)) => (w, f),
            None => (raw.as_str(), ""),
        };
        if whole_s.is_empty()
            || !whole_s.bytes().all(|b| b.is_ascii_digit())
            || !frac_s.bytes().all(|b| b.is_ascii_digit())
            || frac_s.len() > 4
        {
            return Err(AuthzError::ResidencyRegistryInvalid {
                line,
                detail: format!("invalid confidence decimal {raw:?}"),
            });
        }
        let whole: u32 = whole_s
            .parse()
            .map_err(|_| AuthzError::ResidencyRegistryInvalid {
                line,
                detail: format!("invalid confidence integer part {raw:?}"),
            })?;
        let mut frac: u32 = if frac_s.is_empty() {
            0
        } else {
            frac_s
                .parse()
                .map_err(|_| AuthzError::ResidencyRegistryInvalid {
                    line,
                    detail: format!("invalid confidence fractional part {raw:?}"),
                })?
        };
        for _ in frac_s.len()..4 {
            frac *= 10;
        }
        let bps = whole.saturating_mul(10_000).saturating_add(frac);
        if bps > 10_000 {
            return Err(AuthzError::ResidencyRegistryInvalid {
                line,
                detail: format!("confidence {raw} exceeds 1.0"),
            });
        }
        Ok(bps as u16)
    }

    fn into_str_list(self, line: usize, key: &str) -> Result<Vec<String>, AuthzError> {
        match self {
            Self::StrList(list) => Ok(list),
            _ => Err(AuthzError::ResidencyRegistryInvalid {
                line,
                detail: format!("field `{key}` must be a string array"),
            }),
        }
    }
}

fn take_required(
    fields: &mut BTreeMap<String, RawVal>,
    key: &str,
    line: usize,
) -> Result<RawVal, AuthzError> {
    fields
        .remove(key)
        .ok_or_else(|| AuthzError::ResidencyRegistryInvalid {
            line,
            detail: format!("missing required field `{key}`"),
        })
}

fn dup_key(line: usize, key: &str) -> AuthzError {
    AuthzError::ResidencyRegistryInvalid {
        line,
        detail: format!("duplicate key `{key}`"),
    }
}

fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    let mut escaped = false;
    for (i, ch) in line.char_indices() {
        if in_str {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_str = false;
            }
        } else if ch == '"' {
            in_str = true;
        } else if ch == '#' {
            return &line[..i];
        }
    }
    line
}

fn parse_value(raw: &str, line: usize) -> Result<RawVal, AuthzError> {
    if raw.starts_with('"') {
        return Ok(RawVal::Str(parse_quoted_str(raw, line)?));
    }
    if raw.starts_with('[') && raw.ends_with(']') {
        let inner = raw[1..raw.len() - 1].trim();
        if inner.is_empty() {
            return Ok(RawVal::StrList(Vec::new()));
        }
        let mut out = Vec::new();
        let mut rest = inner;
        loop {
            let trimmed = rest.trim_start();
            if trimmed.is_empty() {
                break;
            }
            let (s, after) = take_quoted_prefix(trimmed, line)?;
            out.push(s);
            let after = after.trim_start();
            if after.is_empty() {
                break;
            }
            let Some(next) = after.strip_prefix(',') else {
                return Err(AuthzError::ResidencyRegistryInvalid {
                    line,
                    detail: format!("expected `,` in array near {after:?}"),
                });
            };
            rest = next;
        }
        return Ok(RawVal::StrList(out));
    }
    if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Ok(RawVal::Number(raw.to_string()));
    }
    Err(AuthzError::ResidencyRegistryInvalid {
        line,
        detail: format!("unsupported TOML value {raw:?}"),
    })
}

fn parse_quoted_str(raw: &str, line: usize) -> Result<String, AuthzError> {
    let (s, rest) = take_quoted_prefix(raw, line)?;
    if !rest.trim().is_empty() {
        return Err(AuthzError::ResidencyRegistryInvalid {
            line,
            detail: format!("trailing characters after string: {rest:?}"),
        });
    }
    Ok(s)
}

fn take_quoted_prefix(raw: &str, line: usize) -> Result<(String, &str), AuthzError> {
    let Some(chars) = raw.strip_prefix('"').map(str::char_indices) else {
        return Err(AuthzError::ResidencyRegistryInvalid {
            line,
            detail: format!("expected quoted string at {raw:?}"),
        });
    };
    let after_open = &raw[1..];
    let mut out = String::new();
    let mut escaped = false;
    for (idx, ch) in chars {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => {
                    return Err(AuthzError::ResidencyRegistryInvalid {
                        line,
                        detail: format!("unsupported escape \\{other}"),
                    });
                }
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Ok((out, &after_open[idx + 1..]));
        } else {
            out.push(ch);
        }
    }
    Err(AuthzError::ResidencyRegistryInvalid {
        line,
        detail: "unterminated quoted string".to_string(),
    })
}

fn validate_iso_country(code: &str, line: usize, field: &str) -> Result<(), AuthzError> {
    if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(AuthzError::ResidencyRegistryInvalid {
            line,
            detail: format!(
                "field `{field}` must be 2-letter uppercase ISO country, found {code:?}"
            ),
        });
    }
    Ok(())
}

fn validate_iso_date(date: &str, line: usize, field: &str) -> Result<(), AuthzError> {
    let b = date.as_bytes();
    let ok = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !ok {
        return Err(AuthzError::ResidencyRegistryInvalid {
            line,
            detail: format!("field `{field}` must be YYYY-MM-DD, found {date:?}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_committed_residency_registry_has_72_rows_and_fail_closed_unknown_pair() {
        let reg = ResidencyRegistry::load_from_root(&workspace_root())
            .expect("committed registries/residency.toml parses");
        assert_eq!(reg.len(), 72);
        assert_eq!(reg.residency_epoch, 1);
        assert_eq!(reg.default_rule, ResidencyRule::InCountryOnly);
        // W3 §4 exact counts: 23 in_country_only, 27 aggregates_exportable, 22 exportable.
        assert_eq!(reg.rule_counts(), (23, 27, 22));

        let empty_instruments = BTreeSet::new();

        // BD-094 done-when: unknown (country, data_class) pair ALWAYS fails closed to in_country_only!
        assert_eq!(
            reg.effective_rule("ZZ", DataClass::Aggregate, &empty_instruments),
            ResidencyRule::InCountryOnly
        );
        let unknown_decision =
            reg.evaluate_transfer("ZZ", DataClass::Aggregate, "CA", true, &empty_instruments);
        match unknown_decision {
            ResidencyDecision::Refused(err) => {
                let r = err.to_refusal();
                assert_eq!((r.code, r.exit_code), ("FMINE-REF-303", 7));
            }
            other => panic!("expected FMINE-REF-303 refusal for unknown country, got {other:?}"),
        }

        // Plan §31.2: Pakistan geoscience has unregistered `requires`, so planner holds it in_country_only!
        assert_eq!(
            reg.effective_rule("PK", DataClass::Geoscience, &empty_instruments),
            ResidencyRule::InCountryOnly
        );
        assert!(matches!(
            reg.evaluate_transfer("PK", DataClass::Geoscience, "CA", true, &empty_instruments),
            ResidencyDecision::Refused(AuthzError::ResidencyRefused { .. })
        ));
        let mut with_counsel = BTreeSet::new();
        with_counsel.insert(
            "Counsel reading of r.71(3)(i) + Reko Diq mineral agreement data clause".to_string(),
        );
        assert_eq!(
            reg.effective_rule("PK", DataClass::Geoscience, &with_counsel),
            ResidencyRule::AggregatesExportable
        );
        assert!(matches!(
            reg.evaluate_transfer("PK", DataClass::Geoscience, "CA", true, &with_counsel),
            ResidencyDecision::Allowed { .. }
        ));
        // Even with counsel reading, raw PK geoscience stays in-country.
        assert!(matches!(
            reg.evaluate_transfer("PK", DataClass::Geoscience, "CA", false, &with_counsel),
            ResidencyDecision::Refused(AuthzError::ResidencyRefused { .. })
        ));

        // Zambia personal data (M2 Lumwana): in_country_only cross-border, allowed domestically.
        assert!(matches!(
            reg.evaluate_transfer("ZM", DataClass::Personal, "CA", false, &empty_instruments),
            ResidencyDecision::Refused(AuthzError::ResidencyRefused { .. })
        ));
        assert!(matches!(
            reg.evaluate_transfer("ZM", DataClass::Personal, "ZM", false, &empty_instruments),
            ResidencyDecision::Allowed { .. }
        ));

        // US personal data: exportable to CA, explicitly denied to CN (28 CFR Part 202).
        assert!(matches!(
            reg.evaluate_transfer("US", DataClass::Personal, "CA", false, &empty_instruments),
            ResidencyDecision::Allowed { .. }
        ));
        assert!(matches!(
            reg.evaluate_transfer("US", DataClass::Personal, "CN", false, &empty_instruments),
            ResidencyDecision::Refused(AuthzError::ResidencyRefused { .. })
        ));

        // Fail-open default_rule MUST be refused on parse.
        let fail_open = "schema = \"fmine.residency/1\"\nresidency_epoch = 1\nas_of = \"2026-10-29\"\ndefault_rule = \"exportable\"\n";
        assert!(matches!(
            ResidencyRegistry::parse(fail_open),
            Err(AuthzError::ResidencyRegistryInvalid { .. })
        ));
    }
}
