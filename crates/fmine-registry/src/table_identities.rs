//! Declared PDF table identities (BD-340.S5). Membership, signs, periods and rounding rules are inputs,
//! never inferred from whichever construction makes a table pass. This registry contains no source values.
//! Determinism: Exact (strict parsing and canonical identity/member order).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::sources::{DocumentRow, Fields, SourcesError, fail};
use crate::toml_subset::{TomlValue, parse_subset};

pub mod cutoff;

pub const TABLE_IDENTITIES_SCHEMA: &str = "fmine.table-identities.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    Sum,
    RollForward,
    ReserveReplacement,
    GoldCostPerSoldOunce,
    CopperCostPerSoldPound,
}

impl IdentityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::RollForward => "roll_forward",
            Self::ReserveReplacement => "reserve_replacement",
            Self::GoldCostPerSoldOunce => "gold_cost_per_sold_oz",
            Self::CopperCostPerSoldPound => "copper_cost_per_sold_lb",
        }
    }

    /// Native source scales declared by the cost-rate kind; no mass, FX or ownership conversion.
    #[must_use]
    pub const fn cost_rate(self) -> Option<CostRate> {
        let (denominator_unit, unit, factor) = match self {
            Self::GoldCostPerSoldOunce => ("koz", "usd_per_oz", 1000),
            Self::CopperCostPerSoldPound => ("Mlb", "usd_per_lb", 1),
            _ => return None,
        };
        Some(CostRate {
            numerator_unit: "usd_m",
            denominator_unit,
            unit,
            factor,
        })
    }
}

/// Exact arithmetic implied by a declared sold-metal cost-rate identity, not an accounting definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostRate {
    pub numerator_unit: &'static str,
    pub denominator_unit: &'static str,
    pub unit: &'static str,
    /// `rate = factor * cost / sold`; signed costs retain their printed sign.
    pub factor: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TermRole {
    Member,
    Opening,
    Move,
    Numerator,
    Denominator,
}

impl TermRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Opening => "opening",
            Self::Move => "move",
            Self::Numerator => "numerator",
            Self::Denominator => "denominator",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub quantity: String,
    pub period: String,
    /// Coefficient applied to the signed printed value, never a reinterpretation of its printed sign.
    pub sign: i8,
    pub role: TermRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdentity {
    pub id: String,
    pub doc: String,
    pub family: String,
    pub kind: IdentityKind,
    pub total: String,
    pub total_period: String,
    pub unit: String,
    pub basis: String,
    /// `rounded_once` or `component_sum`; one construction for each family across the whole registry.
    pub construction: String,
    /// `closed`, `half_up`, `half_down`, `half_even` or `truncate`; one convention per document.
    pub convention: String,
    /// Explicit interpretation of a reserve-replacement ratio; absent for every other identity kind.
    pub replacement: Option<Replacement>,
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub definition: String,
    pub numerator_unit: String,
    pub denominator_unit: String,
    /// Percentage scaling. Operand signs are declared independently in their term rows.
    pub factor: i64,
    /// `none` for gold; a named printed deck for already-converted GEO operands. No conversion is performed.
    pub price_deck: String,
}

impl Replacement {
    #[must_use]
    pub fn metal(&self) -> Option<&'static str> {
        let (name, version) = self.definition.split_once('@')?;
        if version.is_empty() || version.chars().any(char::is_whitespace) {
            return None;
        }
        match name {
            "reserve_replacement.gold_oz" => Some("gold"),
            "reserve_replacement.geo" => Some("geo"),
            _ => None,
        }
    }

    fn validate(&self, identity: &TableIdentity) -> Result<(), &'static str> {
        let metal = self
            .metal()
            .ok_or("unsupported named replacement definition")?;
        if self.price_deck.is_empty()
            || !self
                .price_deck
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'@'))
            || (metal == "gold") != (self.price_deck == "none")
        {
            return Err("gold requires deck=none; GEO requires an explicit named price deck");
        }
        let basis = format!(
            "attributable;contained;metal={metal};ex_ad;deck={}",
            self.price_deck
        );
        if identity.basis != basis {
            return Err(
                "replacement basis must explicitly match attributable contained metal, ex-A&D and the named deck",
            );
        }
        let unit = self.numerator_unit.as_str();
        if self.denominator_unit != unit
            || !(matches!(unit, "oz" | "koz" | "moz")
                || (metal == "geo" && matches!(unit, "geo_oz" | "geo_koz" | "geo_moz")))
            || identity.unit != "percent"
            || self.factor != 100
        {
            return Err(
                "replacement requires percent scaling 100 and identical explicit ounce units for both operands",
            );
        }
        let window = identity
            .total_period
            .split_once('-')
            .and_then(|(start, end)| {
                if start.len() != 4 || end.len() != 4 {
                    return None;
                }
                Some((start.parse::<u16>().ok()?, end.parse::<u16>().ok()?))
            });
        if !window.is_some_and(|(start, end)| {
            start >= 1000 && end <= 9999 && start.checked_add(2) == Some(end)
        }) || identity
            .terms
            .iter()
            .any(|term| term.period != identity.total_period)
        {
            return Err(
                "replacement and both cumulative operands require the same three-year YYYY-YYYY window",
            );
        }
        if identity.construction != "rounded_once"
            || identity.terms.len() != 2
            || identity
                .terms
                .iter()
                .filter(|t| t.role == TermRole::Numerator)
                .count()
                != 1
            || identity
                .terms
                .iter()
                .filter(|t| t.role == TermRole::Denominator)
                .count()
                != 1
        {
            return Err(
                "replacement needs rounded_once and exactly one numerator and one denominator",
            );
        }
        Ok(())
    }
}

fn validate_cost_rate(identity: &TableIdentity) -> Result<(), &'static str> {
    let rate = identity
        .kind
        .cost_rate()
        .ok_or("unsupported cost-rate kind")?;
    if identity.unit != rate.unit {
        return Err("cost-rate output unit must match the explicitly selected sold-metal scale");
    }
    if identity.construction != "rounded_once"
        || identity.terms.len() != 2
        || identity
            .terms
            .iter()
            .filter(|t| t.role == TermRole::Numerator)
            .count()
            != 1
        || identity
            .terms
            .iter()
            .filter(|t| t.role == TermRole::Denominator)
            .count()
            != 1
    {
        return Err("cost rate needs rounded_once and exactly one numerator and one denominator");
    }
    if identity
        .terms
        .iter()
        .any(|term| term.sign != 1 || term.period != identity.total_period)
    {
        return Err(
            "cost rate and both operands require the same period and unmodified printed signs",
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdentityRegistry {
    pub issuer: String,
    pub identities: Vec<TableIdentity>,
    pub reserve_rows: Vec<ReserveDeclaration>,
    pub geo_checks: Vec<GeoCheckDeclaration>,
    pub cutoffs: Vec<cutoff::CutoffDeclaration>,
}

/// Explicit GEO membership and optional, source-cited alternative deck hypotheses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoCheckDeclaration {
    pub id: String,
    pub gold_row: String,
    pub copper_row: String,
    pub geo_row: String,
    pub candidate_deck_rows: Vec<String>,
}

impl GeoCheckDeclaration {
    fn validate(&self, rows: &[ReserveDeclaration]) -> Result<(), SourcesError> {
        let bad = |reason: &str| SourcesError {
            line: 0,
            reason: format!("GEO check {}: {reason}", self.id),
        };
        let find = |id: &str| {
            rows.iter()
                .find(|row| row.id == id)
                .ok_or_else(|| bad("every amount and candidate must name a declared reserve row"))
        };
        let (gold, copper, geo) = (
            find(&self.gold_row)?,
            find(&self.copper_row)?,
            find(&self.geo_row)?,
        );
        if gold.metal != "gold"
            || gold.unit != "moz"
            || copper.metal != "copper"
            || copper.unit != "mlb"
            || geo.metal != "geo"
            || geo.unit != "moz"
        {
            return Err(bad(
                "amount rows require gold/GEO in moz and copper in mlb; no implicit rescaling",
            ));
        }
        for row in [gold, copper] {
            if row.doc != geo.doc
                || row.subject != geo.subject
                || row.item != geo.item
                || row.reserve_year != geo.reserve_year
                || row.period != geo.period
                || row.ownership_key() != geo.ownership_key()
                || row.support != geo.support
                || row.convention != geo.convention
            {
                return Err(bad(
                    "amount rows must share document, subject, item, year, period, ownership, support and convention",
                ));
            }
        }
        let mut candidates = BTreeSet::from([&self.geo_row]);
        for id in &self.candidate_deck_rows {
            if !candidates.insert(id) {
                return Err(bad(
                    "candidate deck rows must be distinct and must not repeat the primary GEO row",
                ));
            }
        }
        for id in candidates {
            let row = find(id)?;
            if row.deck != "quoted"
                || row.gold_price_quantity.is_none()
                || row.copper_price_quantity.is_none()
            {
                return Err(bad(
                    "primary and candidate decks require both quoted gold and copper prices",
                ));
            }
        }
        Ok(())
    }
}

/// Interpretation of one issuer assertion; price-deck values themselves remain in agreed source readings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveDeclaration {
    pub id: String,
    pub doc: String,
    pub quantity: String,
    pub subject: String,
    pub item: String,
    pub reserve_year: u16,
    pub period: String,
    pub metal: String,
    pub unit: String,
    pub ownership: String,
    pub support: String,
    pub convention: String,
    pub deck: String,
    pub deck_period: Option<String>,
    pub deck_basis: Option<String>,
    pub gold_price_quantity: Option<String>,
    pub copper_price_quantity: Option<String>,
}

impl ReserveDeclaration {
    /// All three semantics must also be agreed in the source reading's basis, without implicit conversion.
    #[must_use]
    pub fn basis(&self) -> String {
        format!("{};{};{}", self.metal, self.ownership, self.support)
    }

    /// Reduced ownership key; full-site and attributable-at-one remain distinct bases.
    pub fn ownership_key(&self) -> Option<String> {
        if self.ownership == "full" {
            return Some("full".into());
        }
        let (n, d) = self
            .ownership
            .strip_prefix("attributable:")?
            .split_once('/')?;
        if n.is_empty() || d.is_empty() || !n.bytes().chain(d.bytes()).all(|b| b.is_ascii_digit()) {
            return None;
        }
        let (n, d) = (n.parse::<u64>().ok()?, d.parse::<u64>().ok()?);
        if n == 0 || n > d {
            return None;
        }
        let (mut a, mut b) = (n, d);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Some(format!("attributable:{}/{}", n / a, d / a))
    }

    fn validate(&self, documents: &[DocumentRow]) -> Result<(), SourcesError> {
        let bad = |reason: &str| SourcesError {
            line: 0,
            reason: format!("reserve row {}: {reason}", self.id),
        };
        if !key(&self.id)
            || !key(&self.quantity)
            || self.subject.trim().is_empty()
            || self.subject.trim() != self.subject
            || documents.iter().filter(|d| d.file == self.doc).count() != 1
        {
            return Err(bad(
                "declare a unique id, quantity, canonical subject and registered document",
            ));
        }
        if !matches!(
            self.item.as_str(),
            "opening" | "acquisitions_divestments" | "depletion" | "net_change" | "closing"
        ) {
            return Err(bad("unsupported reserve item"));
        }
        let period_year = if self.item == "opening" {
            self.reserve_year.checked_sub(1)
        } else {
            Some(self.reserve_year)
        };
        if !(1..=9999).contains(&self.reserve_year)
            || period_year == Some(0)
            || period_year.map(|y| format!("{y:04}FY")).as_deref() != Some(self.period.as_str())
        {
            return Err(bad(
                "period must identify the reserve year, or its preceding year for opening",
            ));
        }
        let unit_ok = match self.metal.as_str() {
            "gold" | "geo" => matches!(self.unit.as_str(), "moz" | "koz"),
            "copper" => matches!(self.unit.as_str(), "mlb" | "kt"),
            _ => false,
        };
        if !unit_ok
            || !matches!(self.support.as_str(), "contained" | "recoverable")
            || self.ownership_key().is_none()
        {
            return Err(bad(
                "metal, unit, support and full or attributable ownership must be explicit",
            ));
        }
        if !matches!(
            self.convention.as_str(),
            "closed" | "half_up" | "half_down" | "half_even" | "truncate"
        ) {
            return Err(bad("unknown rounding convention"));
        }
        match self.deck.as_str() {
            "unstated" => {
                if self.deck_period.is_some()
                    || self.deck_basis.is_some()
                    || self.gold_price_quantity.is_some()
                    || self.copper_price_quantity.is_some()
                {
                    return Err(bad("an unstated deck cannot supply price fields"));
                }
            }
            "quoted" => {
                if self
                    .deck_period
                    .as_deref()
                    .is_none_or(|s| s.trim().is_empty())
                    || self
                        .deck_basis
                        .as_deref()
                        .is_none_or(|s| s.trim().is_empty())
                    || (matches!(self.metal.as_str(), "gold" | "geo")
                        && self.gold_price_quantity.is_none())
                    || (matches!(self.metal.as_str(), "copper" | "geo")
                        && self.copper_price_quantity.is_none())
                {
                    return Err(bad(
                        "a quoted deck needs a period, basis and the metal's price quantities",
                    ));
                }
                let mut quantities = BTreeSet::from([self.quantity.as_str()]);
                for q in [&self.gold_price_quantity, &self.copper_price_quantity]
                    .into_iter()
                    .flatten()
                {
                    if !key(q) || !quantities.insert(q) {
                        return Err(bad("price quantities must be distinct keys"));
                    }
                }
            }
            _ => return Err(bad("deck must be quoted or unstated")),
        }
        Ok(())
    }
}

fn key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl TableIdentityRegistry {
    /// Complete source selection for all declared arithmetic, reserve assertions and quoted price decks.
    #[must_use]
    pub fn quantities_by_document(&self) -> BTreeMap<String, Vec<String>> {
        let mut selected: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for identity in &self.identities {
            let quantities = selected.entry(identity.doc.clone()).or_default();
            quantities.insert(identity.total.clone());
            quantities.extend(identity.terms.iter().map(|t| t.quantity.clone()));
        }
        for row in &self.reserve_rows {
            let quantities = selected.entry(row.doc.clone()).or_default();
            quantities.insert(row.quantity.clone());
            quantities.extend(
                [&row.gold_price_quantity, &row.copper_price_quantity]
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        for cutoff in &self.cutoffs {
            selected
                .entry(cutoff.doc.clone())
                .or_default()
                .extend(cutoff.inputs.iter().map(|input| input.quantity.clone()));
        }
        selected
            .into_iter()
            .map(|(doc, keys)| (doc, keys.into_iter().collect()))
            .collect()
    }

    /// Rechecks public structs at a consumer boundary, including caller-constructed declarations.
    pub fn validate(&self, documents: &[DocumentRow]) -> Result<(), SourcesError> {
        if !key(&self.issuer)
            || (self.identities.is_empty()
                && self.reserve_rows.is_empty()
                && self.cutoffs.is_empty())
        {
            return fail(
                0,
                "an issuer key and at least one table identity, reserve row or cutoff are required",
            );
        }
        let mut ids = BTreeSet::new();
        let mut families = BTreeMap::new();
        let mut conventions = BTreeMap::new();
        for identity in &self.identities {
            let bad = |detail: &str| SourcesError {
                line: 0,
                reason: format!("identity {}: {detail}", identity.id),
            };
            if !key(&identity.id) || !ids.insert(&identity.id) || !key(&identity.family) {
                return Err(bad("id and family must be keys; ids must be unique"));
            }
            if documents.iter().filter(|d| d.file == identity.doc).count() != 1 {
                return Err(bad("document must name exactly one registered document"));
            }
            if identity.unit.trim().is_empty()
                || identity.basis.trim().is_empty()
                || identity.total_period.trim().is_empty()
                || !key(&identity.total)
            {
                return Err(bad(
                    "total quantity, period, unit and basis must be explicit",
                ));
            }
            if !matches!(
                identity.construction.as_str(),
                "rounded_once" | "component_sum"
            ) {
                return Err(bad("unknown rounding construction"));
            }
            if !matches!(
                identity.convention.as_str(),
                "closed" | "half_up" | "half_down" | "half_even" | "truncate"
            ) {
                return Err(bad("unknown rounding convention"));
            }
            if families
                .insert(&identity.family, &identity.construction)
                .is_some_and(|old| old != &identity.construction)
            {
                return Err(bad(
                    "a family cannot choose different rounding constructions",
                ));
            }
            if conventions
                .insert(&identity.doc, &identity.convention)
                .is_some_and(|old| old != &identity.convention)
            {
                return Err(bad("a document cannot choose different tie conventions"));
            }
            let mut quantities = BTreeSet::from([&identity.total]);
            if identity.terms.is_empty() {
                return Err(bad("membership must be declared with term rows"));
            }
            for term in &identity.terms {
                if !key(&term.quantity)
                    || !quantities.insert(&term.quantity)
                    || term.period.trim().is_empty()
                    || !matches!(term.sign, -1 | 1)
                {
                    return Err(bad(
                        "terms need unique quantities, explicit periods and signs of -1 or 1",
                    ));
                }
                if !matches!(
                    (identity.kind, term.role),
                    (IdentityKind::Sum, TermRole::Member)
                        | (
                            IdentityKind::RollForward,
                            TermRole::Opening | TermRole::Move
                        )
                        | (
                            IdentityKind::ReserveReplacement
                                | IdentityKind::GoldCostPerSoldOunce
                                | IdentityKind::CopperCostPerSoldPound,
                            TermRole::Numerator | TermRole::Denominator
                        )
                ) || (term.role == TermRole::Opening && term.sign != 1)
                {
                    return Err(bad("term role does not match the declared identity kind"));
                }
            }
            if identity.kind == IdentityKind::RollForward
                && (identity
                    .terms
                    .iter()
                    .filter(|t| t.role == TermRole::Opening)
                    .count()
                    != 1
                    || !identity.terms.iter().any(|t| t.role == TermRole::Move))
            {
                return Err(bad(
                    "a roll-forward needs one positive opening and at least one movement",
                ));
            }
            let quoted_rate_reused = || {
                self.identities.iter().any(|other| {
                    other.doc == identity.doc
                        && other.id != identity.id
                        && (other.total == identity.total
                            || other.terms.iter().any(|t| t.quantity == identity.total))
                })
            };
            match (&identity.replacement, identity.kind) {
                (Some(replacement), IdentityKind::ReserveReplacement) => {
                    replacement.validate(identity).map_err(bad)?;
                    if quoted_rate_reused() {
                        return Err(bad(
                            "a quoted replacement rate cannot be reused in another identity; shared-rate bilinear checks are unsupported",
                        ));
                    }
                }
                (None, kind) if kind.cost_rate().is_some() => {
                    validate_cost_rate(identity).map_err(bad)?;
                    if quoted_rate_reused() {
                        return Err(bad(
                            "a quoted cost rate cannot be reused in another identity; shared-rate bilinear checks are unsupported",
                        ));
                    }
                }
                (None, IdentityKind::Sum | IdentityKind::RollForward) => {}
                _ => {
                    return Err(bad(
                        "replacement metadata is required only for reserve_replacement identities",
                    ));
                }
            }
        }
        let mut source_keys = BTreeSet::new();
        for row in &self.reserve_rows {
            row.validate(documents)?;
            if conventions
                .insert(&row.doc, &row.convention)
                .is_some_and(|old| old != &row.convention)
            {
                return fail(
                    0,
                    "a document cannot choose different tie conventions across identities and reserve rows",
                );
            }
            if !ids.insert(&row.id) || !source_keys.insert((&row.doc, &row.quantity)) {
                return fail(
                    0,
                    "reserve rows require unique ids and unique document/quantity pairs",
                );
            }
        }
        for check in &self.geo_checks {
            if !key(&check.id) || !ids.insert(&check.id) {
                return fail(0, "GEO checks require unique ids across all declarations");
            }
            check.validate(&self.reserve_rows)?;
        }
        for cutoff in &self.cutoffs {
            if !ids.insert(&cutoff.id) {
                return fail(0, "cutoffs require unique ids across all declarations");
            }
            cutoff.validate(documents)?;
        }
        Ok(())
    }
}

/// Parses explicit `[[identity]]` and linked `[[term]]` rows using the strict registry TOML subset.
pub fn parse_table_identities(
    text: &str,
    issuer: &str,
    documents: &[DocumentRow],
) -> Result<TableIdentityRegistry, SourcesError> {
    let parsed = parse_subset(text)?;
    let top = Fields {
        fields: &parsed.top,
        line: 0,
        what: "the top level".to_string(),
    };
    top.only(&["schema", "issuer"])?;
    let (schema, line) = top.string("schema")?;
    if schema != TABLE_IDENTITIES_SCHEMA {
        return fail(line, format!("schema must be {TABLE_IDENTITIES_SCHEMA:?}"));
    }
    let (named, line) = top.string("issuer")?;
    if named != issuer {
        return fail(line, "issuer does not match the sources registry");
    }
    let mut identities = Vec::new();
    let mut reserve_rows = Vec::new();
    let mut geo_checks = Vec::new();
    let mut geo_candidates = Vec::new();
    let mut terms = Vec::new();
    for row in &parsed.rows {
        let f = Fields::of_row(row);
        match row.table.as_str() {
            "cutoff" | "cutoff_input" => {}
            "geo_check" => {
                f.only(&["id", "gold_row", "copper_row", "geo_row"])?;
                geo_checks.push(GeoCheckDeclaration {
                    id: f.string("id")?.0,
                    gold_row: f.string("gold_row")?.0,
                    copper_row: f.string("copper_row")?.0,
                    geo_row: f.string("geo_row")?.0,
                    candidate_deck_rows: Vec::new(),
                });
            }
            "geo_candidate" => {
                f.only(&["check", "row"])?;
                geo_candidates.push((f.string("check")?.0, f.string("row")?.0, row.line));
            }
            "reserve_row" => {
                f.only(&[
                    "id",
                    "doc",
                    "quantity",
                    "subject",
                    "item",
                    "reserve_year",
                    "period",
                    "metal",
                    "unit",
                    "ownership",
                    "support",
                    "convention",
                    "deck",
                    "deck_period",
                    "deck_basis",
                    "gold_price_quantity",
                    "copper_price_quantity",
                ])?;
                let optional = |name: &str| {
                    if row.get(name).is_some() {
                        f.string(name).map(|(s, _)| Some(s))
                    } else {
                        Ok(None)
                    }
                };
                let (year, line) = f.positive_int("reserve_year")?;
                let reserve_year = u16::try_from(year).map_err(|_| SourcesError {
                    line,
                    reason: "reserve_year is too large".into(),
                })?;
                reserve_rows.push(ReserveDeclaration {
                    id: f.string("id")?.0,
                    doc: f.string("doc")?.0,
                    quantity: f.string("quantity")?.0,
                    subject: f.string("subject")?.0,
                    item: f.string("item")?.0,
                    reserve_year,
                    period: f.string("period")?.0,
                    metal: f.string("metal")?.0,
                    unit: f.string("unit")?.0,
                    ownership: f.string("ownership")?.0,
                    support: f.string("support")?.0,
                    convention: f.string("convention")?.0,
                    deck: f.string("deck")?.0,
                    deck_period: optional("deck_period")?,
                    deck_basis: optional("deck_basis")?,
                    gold_price_quantity: optional("gold_price_quantity")?,
                    copper_price_quantity: optional("copper_price_quantity")?,
                });
            }
            "identity" => {
                f.only(&[
                    "id",
                    "doc",
                    "family",
                    "kind",
                    "total",
                    "total_period",
                    "unit",
                    "basis",
                    "construction",
                    "convention",
                    "definition",
                    "numerator_unit",
                    "denominator_unit",
                    "factor",
                    "price_deck",
                ])?;
                let (kind, line) = f.string("kind")?;
                let kind = match kind.as_str() {
                    "sum" => IdentityKind::Sum,
                    "roll_forward" => IdentityKind::RollForward,
                    "reserve_replacement" => IdentityKind::ReserveReplacement,
                    "gold_cost_per_sold_oz" => IdentityKind::GoldCostPerSoldOunce,
                    "copper_cost_per_sold_lb" => IdentityKind::CopperCostPerSoldPound,
                    _ => {
                        return fail(
                            line,
                            "kind must be sum, roll_forward, reserve_replacement, gold_cost_per_sold_oz or copper_cost_per_sold_lb",
                        );
                    }
                };
                let replacement = if kind == IdentityKind::ReserveReplacement {
                    let Some(TomlValue::Int(factor)) = row.get("factor").map(|v| &v.value) else {
                        return fail(row.line, "replacement factor must be an explicit integer");
                    };
                    Some(Replacement {
                        definition: f.string("definition")?.0,
                        numerator_unit: f.string("numerator_unit")?.0,
                        denominator_unit: f.string("denominator_unit")?.0,
                        factor: *factor,
                        price_deck: f.string("price_deck")?.0,
                    })
                } else {
                    if [
                        "definition",
                        "numerator_unit",
                        "denominator_unit",
                        "factor",
                        "price_deck",
                    ]
                    .iter()
                    .any(|key| row.get(key).is_some())
                    {
                        return fail(
                            row.line,
                            "replacement fields cannot decorate another identity kind",
                        );
                    }
                    None
                };
                identities.push(TableIdentity {
                    id: f.string("id")?.0,
                    doc: f.string("doc")?.0,
                    family: f.string("family")?.0,
                    kind,
                    total: f.string("total")?.0,
                    total_period: f.string("total_period")?.0,
                    unit: f.string("unit")?.0,
                    basis: f.string("basis")?.0,
                    construction: f.string("construction")?.0,
                    convention: f.string("convention")?.0,
                    replacement,
                    terms: Vec::new(),
                });
            }
            "term" => {
                f.only(&["identity", "quantity", "period", "sign", "role"])?;
                let sign = match row.get("sign").map(|v| &v.value) {
                    Some(TomlValue::Int(-1)) => -1,
                    Some(TomlValue::Int(1)) => 1,
                    _ => return fail(row.line, "term sign must be the integer -1 or 1"),
                };
                let (role, line) = f.string("role")?;
                let role = match role.as_str() {
                    "member" => TermRole::Member,
                    "opening" => TermRole::Opening,
                    "move" => TermRole::Move,
                    "numerator" => TermRole::Numerator,
                    "denominator" => TermRole::Denominator,
                    _ => {
                        return fail(
                            line,
                            "role must be member, opening, move, numerator or denominator",
                        );
                    }
                };
                terms.push((
                    f.string("identity")?.0,
                    row.line,
                    Term {
                        quantity: f.string("quantity")?.0,
                        period: f.string("period")?.0,
                        sign,
                        role,
                    },
                ));
            }
            _ => return fail(row.line, format!("unknown table [[{}]]", row.table)),
        }
    }
    for (id, line, term) in terms {
        let Some(identity) = identities.iter_mut().find(|v| v.id == id) else {
            return fail(line, format!("term names unknown identity {id:?}"));
        };
        identity.terms.push(term);
    }
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    for identity in &mut identities {
        identity.terms.sort_by(|a, b| a.quantity.cmp(&b.quantity));
    }
    for (id, row, line) in geo_candidates {
        let Some(check) = geo_checks.iter_mut().find(|check| check.id == id) else {
            return fail(line, format!("GEO candidate names unknown check {id:?}"));
        };
        check.candidate_deck_rows.push(row);
    }
    geo_checks.sort_by(|a, b| a.id.cmp(&b.id));
    for check in &mut geo_checks {
        check.candidate_deck_rows.sort();
    }
    let registry = TableIdentityRegistry {
        issuer: named,
        identities,
        reserve_rows,
        geo_checks,
        cutoffs: cutoff::parse(&parsed)?,
    };
    registry.validate(documents)?;
    Ok(registry)
}

pub fn load_table_identities(
    path: &Path,
    issuer: &str,
    documents: &[DocumentRow],
) -> Result<TableIdentityRegistry, SourcesError> {
    let text = std::fs::read_to_string(path).map_err(|e| SourcesError {
        line: 0,
        reason: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_table_identities(&text, issuer, documents)
}

#[cfg(test)]
mod tests {
    include!("../../xtask/testdata/registry_table_identities_controls.rs");
}
