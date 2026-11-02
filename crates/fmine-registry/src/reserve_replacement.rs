//! Explicit source selection for reserve replacement (BD-063). This registry contains selectors,
//! never issuer values. A complete annual population and the sign convention are fixed before scoring.
//! Determinism: Exact (canonical series/year order and strict TOML parsing).

use std::collections::{BTreeMap, BTreeSet};

use crate::sources::{Fields, SourcesError, fail};
use crate::toml_subset::{TomlRow, TomlValue, parse_subset};

pub const REPLACEMENT_SCHEMA: &str = "fmine.reserve-replacement.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepletionSign {
    Negative,
    Positive,
}

impl DepletionSign {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Negative => "negative",
            Self::Positive => "positive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlineKind {
    Rounded,
    /// Words such as "close to" supply no numerical tolerance that the engine may invent.
    Approximate,
}

impl HeadlineKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rounded => "rounded",
            Self::Approximate => "approximate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearSelection {
    pub year: u16,
    pub doc: String,
    pub net_change: String,
    pub depletion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headline {
    pub doc: String,
    pub quantity: String,
    pub period: String,
    pub kind: HeadlineKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregate {
    pub doc: String,
    pub net_change: String,
    pub depletion: String,
    pub period: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Series {
    pub id: String,
    pub definition: String,
    pub start_year: u16,
    pub end_year: u16,
    pub unit: String,
    pub basis: String,
    pub depletion_sign: DepletionSign,
    pub years: Vec<YearSelection>,
    pub aggregate: Option<Aggregate>,
    pub headline: Option<Headline>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementRegistry {
    pub issuer: String,
    pub series: Vec<Series>,
}

fn key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn year(row: &TomlRow, name: &str) -> Result<u16, SourcesError> {
    match row.get(name).map(|f| &f.value) {
        Some(TomlValue::Int(value)) if (1..=9999).contains(value) => Ok(*value as u16),
        _ => fail(
            row.line,
            format!("{name} must be an integer year in 1..9999"),
        ),
    }
}

impl ReplacementRegistry {
    /// Rechecks public structs at the numerical boundary. Missing years cannot become a smaller sample.
    pub fn validate(&self) -> Result<(), SourcesError> {
        if !key(&self.issuer) || self.series.is_empty() {
            return fail(
                0,
                "an issuer key and at least one reserve series are required",
            );
        }
        let mut ids = BTreeSet::new();
        for series in &self.series {
            let bad = |why: &str| SourcesError {
                line: 0,
                reason: format!("reserve series {}: {why}", series.id),
            };
            if !key(&series.id) || !ids.insert(&series.id) {
                return Err(bad("series ids must be unique keys"));
            }
            if series.definition.trim().is_empty()
                || series.unit.trim().is_empty()
                || series.basis.trim().is_empty()
            {
                return Err(bad("definition, unit and basis must be explicit"));
            }
            if series.start_year == 0
                || series.start_year > series.end_year
                || series.end_year > 9999
                || series.end_year - series.start_year >= 100
            {
                return Err(bad(
                    "the annual window must contain between one and one hundred years",
                ));
            }
            let mut years = BTreeSet::new();
            let mut quantities = BTreeSet::new();
            for selected in &series.years {
                if !(series.start_year..=series.end_year).contains(&selected.year)
                    || !years.insert(selected.year)
                {
                    return Err(bad(
                        "annual selections must be unique and inside the stated window",
                    ));
                }
                if selected.doc.trim().is_empty()
                    || !key(&selected.net_change)
                    || !key(&selected.depletion)
                    || !quantities.insert((&selected.doc, &selected.net_change))
                    || !quantities.insert((&selected.doc, &selected.depletion))
                {
                    return Err(bad(
                        "each annual operand needs a unique document/quantity selector",
                    ));
                }
            }
            if years.len() != usize::from(series.end_year - series.start_year) + 1 {
                return Err(bad(
                    "every year in the stated window needs an explicit annual selection",
                ));
            }
            let period = format!("{}-{}", series.start_year, series.end_year);
            if let Some(aggregate) = &series.aggregate
                && (aggregate.doc.trim().is_empty()
                    || !key(&aggregate.net_change)
                    || !key(&aggregate.depletion)
                    || aggregate.period != period
                    || !quantities.insert((&aggregate.doc, &aggregate.net_change))
                    || !quantities.insert((&aggregate.doc, &aggregate.depletion)))
            {
                return Err(bad(
                    "aggregate totals need distinct selectors and the exact start_year-end_year period",
                ));
            }
            if let Some(headline) = &series.headline
                && (headline.doc.trim().is_empty()
                    || !key(&headline.quantity)
                    || headline.period != period
                    || !quantities.insert((&headline.doc, &headline.quantity)))
            {
                return Err(bad(
                    "the headline needs its own selector and the exact start_year-end_year period",
                ));
            }
        }
        Ok(())
    }
}

/// Parses source selectors, preserving missing source values for the consumer's explicit refusal report.
pub fn parse_replacement_registry(
    text: &str,
    issuer: &str,
) -> Result<ReplacementRegistry, SourcesError> {
    let parsed = parse_subset(text)?;
    let top = Fields {
        fields: &parsed.top,
        line: 0,
        what: "the top level".into(),
    };
    top.only(&["schema", "issuer"])?;
    let (schema, line) = top.string("schema")?;
    if schema != REPLACEMENT_SCHEMA {
        return fail(line, format!("schema must be {REPLACEMENT_SCHEMA:?}"));
    }
    let (named, line) = top.string("issuer")?;
    if named != issuer {
        return fail(line, "issuer does not match the sources registry");
    }
    let mut series = BTreeMap::new();
    let mut years = Vec::new();
    let mut aggregates = Vec::new();
    let mut headlines = Vec::new();
    for row in &parsed.rows {
        let fields = Fields::of_row(row);
        match row.table.as_str() {
            "series" => {
                fields.only(&[
                    "id",
                    "definition",
                    "start_year",
                    "end_year",
                    "unit",
                    "basis",
                    "depletion_sign",
                ])?;
                let (sign, line) = fields.string("depletion_sign")?;
                let depletion_sign = match sign.as_str() {
                    "negative" => DepletionSign::Negative,
                    "positive" => DepletionSign::Positive,
                    _ => return fail(line, "depletion_sign must be negative or positive"),
                };
                let item = Series {
                    id: fields.string("id")?.0,
                    definition: fields.string("definition")?.0,
                    start_year: year(row, "start_year")?,
                    end_year: year(row, "end_year")?,
                    unit: fields.string("unit")?.0,
                    basis: fields.string("basis")?.0,
                    depletion_sign,
                    years: Vec::new(),
                    aggregate: None,
                    headline: None,
                };
                if series.insert(item.id.clone(), item).is_some() {
                    return fail(row.line, "duplicate reserve series id");
                }
            }
            "year" => {
                fields.only(&["series", "year", "doc", "net_change", "depletion"])?;
                years.push((
                    fields.string("series")?.0,
                    row.line,
                    YearSelection {
                        year: year(row, "year")?,
                        doc: fields.string("doc")?.0,
                        net_change: fields.string("net_change")?.0,
                        depletion: fields.string("depletion")?.0,
                    },
                ));
            }
            "headline" => {
                fields.only(&["series", "doc", "quantity", "period", "kind"])?;
                let (kind, line) = fields.string("kind")?;
                let kind = match kind.as_str() {
                    "rounded" => HeadlineKind::Rounded,
                    "approximate" => HeadlineKind::Approximate,
                    _ => return fail(line, "headline kind must be rounded or approximate"),
                };
                headlines.push((
                    fields.string("series")?.0,
                    row.line,
                    Headline {
                        doc: fields.string("doc")?.0,
                        quantity: fields.string("quantity")?.0,
                        period: fields.string("period")?.0,
                        kind,
                    },
                ));
            }
            "aggregate" => {
                fields.only(&["series", "doc", "net_change", "depletion", "period"])?;
                aggregates.push((
                    fields.string("series")?.0,
                    row.line,
                    Aggregate {
                        doc: fields.string("doc")?.0,
                        net_change: fields.string("net_change")?.0,
                        depletion: fields.string("depletion")?.0,
                        period: fields.string("period")?.0,
                    },
                ));
            }
            _ => return fail(row.line, format!("unknown table [[{}]]", row.table)),
        }
    }
    for (id, line, selected) in years {
        let Some(item) = series.get_mut(&id) else {
            return fail(line, format!("year names unknown reserve series {id:?}"));
        };
        item.years.push(selected);
    }
    for (id, line, headline) in headlines {
        let Some(item) = series.get_mut(&id) else {
            return fail(
                line,
                format!("headline names unknown reserve series {id:?}"),
            );
        };
        if item.headline.replace(headline).is_some() {
            return fail(
                line,
                format!("duplicate headline for reserve series {id:?}"),
            );
        }
    }
    for (id, line, aggregate) in aggregates {
        let Some(item) = series.get_mut(&id) else {
            return fail(
                line,
                format!("aggregate names unknown reserve series {id:?}"),
            );
        };
        if item.aggregate.replace(aggregate).is_some() {
            return fail(
                line,
                format!("duplicate aggregate for reserve series {id:?}"),
            );
        }
    }
    let registry = ReplacementRegistry {
        issuer: named,
        series: series
            .into_values()
            .map(|mut item| {
                item.years.sort_by_key(|selected| selected.year);
                item
            })
            .collect(),
    };
    registry.validate()?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        let mut out = "schema = \"fmine.reserve-replacement.v1\"\nissuer = \"synthetic\"\n\
            [[series]]\nid = \"gold\"\ndefinition = \"reserve.gold@synthetic\"\n\
            start_year = 2030\nend_year = 2032\nunit = \"Moz\"\n\
            basis = \"attributable_gold_ex_ad\"\ndepletion_sign = \"negative\"\n"
            .to_string();
        for year in [2032, 2030, 2031] {
            out.push_str(&format!(
                "[[year]]\nseries = \"gold\"\nyear = {year}\ndoc = \"Report.pdf\"\n\
                 net_change = \"net_{year}\"\ndepletion = \"depletion_{year}\"\n"
            ));
        }
        out.push_str(
            "[[headline]]\nseries = \"gold\"\ndoc = \"Report.pdf\"\n\
            quantity = \"headline\"\nperiod = \"2030-2032\"\nkind = \"approximate\"\n",
        );
        out.push_str(
            "[[aggregate]]\nseries = \"gold\"\ndoc = \"Report.pdf\"\n\
            net_change = \"total_net\"\ndepletion = \"total_depletion\"\nperiod = \"2030-2032\"\n",
        );
        out
    }

    #[test]
    fn complete_population_is_canonical_and_qualitative_headlines_stay_explicit() {
        let parsed = parse_replacement_registry(&fixture(), "synthetic").unwrap();
        assert_eq!(
            parsed.series[0]
                .years
                .iter()
                .map(|r| r.year)
                .collect::<Vec<_>>(),
            [2030, 2031, 2032]
        );
        assert_eq!(
            parsed.series[0].headline.as_ref().unwrap().kind,
            HeadlineKind::Approximate
        );
        let mut missing = parsed.clone();
        missing.series[0].years.pop();
        assert!(
            missing
                .validate()
                .unwrap_err()
                .reason
                .contains("every year")
        );
        let mut duplicate = parsed.clone();
        duplicate.series[0].years[1].year = 2030;
        assert!(duplicate.validate().is_err());
        let mut reused = parsed.clone();
        reused.series[0].years[0].net_change = "depletion_2030".into();
        assert!(reused.validate().is_err());
        let mut reused_total = parsed.clone();
        reused_total.series[0]
            .aggregate
            .as_mut()
            .unwrap()
            .net_change = "net_2030".into();
        assert!(reused_total.validate().is_err());
    }

    #[test]
    fn malformed_selectors_cannot_silently_change_the_window_or_operand_meaning() {
        for text in [
            fixture().replace("end_year = 2032", "end_year = 2033"),
            fixture().replace("year = 2031", "year = 2029"),
            fixture().replace(
                "depletion_sign = \"negative\"",
                "depletion_sign = \"absolute\"",
            ),
            fixture().replace("kind = \"approximate\"", "kind = \"roughly\""),
            fixture().replace("period = \"2030-2032\"", "period = \"2028-2032\""),
            fixture().replace("net_change =", "net_additions ="),
            fixture().replace("series = \"gold\"", "series = \"missing\""),
            fixture().replace("[[headline]]", "[[headlines]]"),
            fixture().replace("issuer = \"synthetic\"", "issuer = \"different\""),
        ] {
            assert!(
                parse_replacement_registry(&text, "synthetic").is_err(),
                "{text}"
            );
        }
    }
}
