//! Issuer reserve assertions with independent price-deck and filing-vintage keys (BD-335).
//! Determinism: Exact (rational rounding bounds and canonical source, declaration and selection ordering).
//!
//! Callers authenticate source bytes and reader agreement. Current means latest declared eligible filing,
//! never true reserve: older assertions and deck cross-checks remain readable, and incomplete newer filings
//! block fallback to an older one. No classification or EstimateStatement is constructed here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::hash::sha256;
use fmine_evidence::EvidenceClass;
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_registry::sources::{DocumentRow, is_date};
use fmine_registry::table_identities::{ReserveDeclaration, TableIdentityRegistry};
use fmine_registry::transcriptions::Accepted;

use crate::roll_forward::{
    SourceCell, accepted_json, bind_reading, canonical_readings, convention, definition_json,
    interval, js, rat,
};

pub mod geo;

pub const NO_CLAIM: &str = "Issuer assertions from the declared registered source population, not classified reserves or independently verified physical quantities. Current means latest declared eligible MD&A or AIF filing; it is not a truth judgment or proof of complete filing coverage. Investor decks are cross-checks only. Changed or unstated price decks do not establish a reserve restatement.";

/// The economic comparison key deliberately excludes the price deck and filing vintage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReserveKey {
    pub issuer: String,
    pub subject: String,
    pub item: String,
    pub reserve_year: u16,
    pub metal: String,
    pub unit: String,
    pub ownership: String,
    pub support: String,
}

impl ReserveKey {
    fn of(issuer: &str, row: &ReserveDeclaration) -> Self {
        Self {
            issuer: issuer.into(),
            subject: row.subject.clone(),
            item: row.item.clone(),
            reserve_year: row.reserve_year,
            metal: row.metal.clone(),
            unit: row.unit.clone(),
            ownership: row.ownership_key().unwrap_or_else(|| row.ownership.clone()),
            support: row.support.clone(),
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"issuer\":{},\"subject\":{},\"item\":{},\"reserve_year\":{},\"metal\":{},\"unit\":{},\"ownership\":{},\"support\":{}}}",
            js(&self.issuer),
            js(&self.subject),
            js(&self.item),
            self.reserve_year,
            js(&self.metal),
            js(&self.unit),
            js(&self.ownership),
            js(&self.support)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRole {
    FiledReport,
    InvestorDeck,
}

impl SourceRole {
    fn label(self) -> &'static str {
        match self {
            Self::FiledReport => "filed_report",
            Self::InvestorDeck => "investor_deck",
        }
    }
}

/// The exact quoted prices are parameters, not estimates normalized across filings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveDeck {
    /// None for an unstated deck: absence never asserts that two sets of assumptions agree.
    pub key: Option<String>,
    pub period: Option<String>,
    pub basis: Option<String>,
    pub gold_price: Option<SourceCell>,
    pub copper_price: Option<SourceCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerReserveRow {
    declaration: ReserveDeclaration,
    economic_key: ReserveKey,
    source_role: SourceRole,
    source: SourceCell,
    interval: Interval,
    deck: ReserveDeck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveRefusal {
    pub declaration: ReserveDeclaration,
    pub document: DocumentRow,
    pub reason: String,
    pub evidence: Vec<Accepted>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSelection {
    pub economic_key: ReserveKey,
    pub status: &'static str,
    pub latest_filed_date: Option<String>,
    pub current_id: Option<String>,
    /// Every declared eligible row on the latest filing date, before filtering unusable readings.
    pub candidates: Vec<String>,
    pub unavailable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VintageComparison {
    pub left: String,
    pub right: String,
    pub status: &'static str,
    pub overlap: Option<Interval>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveReport {
    pub issuer: String,
    pub rows: Vec<IssuerReserveRow>,
    pub selections: Vec<CurrentSelection>,
    pub comparisons: Vec<VintageComparison>,
    pub geo_checks: Vec<geo::GeoCheckReport>,
    pub refused: Vec<ReserveRefusal>,
    pub registry_refusal: Option<String>,
    /// Binds the complete supplied population when invalid registry metadata prevents row binding.
    pub refused_input_digest: Option<String>,
}

fn role(document: &DocumentRow) -> Result<SourceRole, String> {
    match document.kind.as_str() {
        "mda" | "aif" => Ok(SourceRole::FiledReport),
        "investor_deck" => Ok(SourceRole::InvestorDeck),
        other => Err(format!(
            "document kind {other} is not an eligible issuer reserve filing or investor-deck cross-check"
        )),
    }
}

fn bind(
    issuer: &str,
    row: &ReserveDeclaration,
    readings: &[Accepted],
    document: &DocumentRow,
) -> Result<IssuerReserveRow, String> {
    let source_role = role(document)?;
    let source = bind_reading(
        &row.doc,
        &row.quantity,
        &row.period,
        &row.unit,
        &row.basis(),
        readings,
        document,
    )?;
    let half = source
        .cell
        .step
        .checked_div(Rat::from_integer(2))
        .map_err(|e| e.to_string())?;
    let bounds = convention(&row.convention)
        .interval(source.cell.printed, half)
        .map_err(|e| format!("rounding interval unavailable: {e:?}"))?;
    let deck = bind_deck(row, readings, document)?;
    Ok(IssuerReserveRow {
        declaration: row.clone(),
        economic_key: ReserveKey::of(issuer, row),
        source_role,
        source,
        interval: bounds,
        deck,
    })
}

// A candidate deck is a price hypothesis; its availability does not depend on an unrelated reserve amount.
fn bind_deck(
    row: &ReserveDeclaration,
    readings: &[Accepted],
    document: &DocumentRow,
) -> Result<ReserveDeck, String> {
    role(document)?;
    let mut deck = ReserveDeck {
        key: None,
        period: row.deck_period.clone(),
        basis: row.deck_basis.clone(),
        gold_price: None,
        copper_price: None,
    };
    if row.deck == "quoted" {
        let period = row
            .deck_period
            .as_deref()
            .ok_or("quoted deck lacks period")?;
        let basis = row.deck_basis.as_deref().ok_or("quoted deck lacks basis")?;
        let price = |quantity: &str, unit: &str| -> Result<SourceCell, String> {
            let p = bind_reading(&row.doc, quantity, period, unit, basis, readings, document)?;
            if p.cell.printed.signum() <= 0 {
                return Err(format!(
                    "{}: reserve-deck price must be positive",
                    p.source.locator
                ));
            }
            Ok(p)
        };
        deck.gold_price = row
            .gold_price_quantity
            .as_deref()
            .map(|q| price(q, "usd_per_oz"))
            .transpose()?;
        deck.copper_price = row
            .copper_price_quantity
            .as_deref()
            .map(|q| price(q, "usd_per_lb"))
            .transpose()?;
        let value = |p: &Option<SourceCell>| {
            p.as_ref()
                .map_or_else(|| "null".into(), |p| rat(p.cell.printed))
        };
        let canonical = format!(
            "fmine.reserve-deck/1\n{}\n{}\n{}\n{}",
            js(period),
            js(basis),
            value(&deck.gold_price),
            value(&deck.copper_price)
        );
        deck.key = Some(sha256(canonical.as_bytes()).to_hex());
    }
    Ok(deck)
}

fn evidence(row: &ReserveDeclaration, readings: &[Accepted]) -> Vec<Accepted> {
    let mut out: Vec<_> = readings
        .iter()
        .filter(|a| {
            a.locator.doc == row.doc
                && (a.quantity == row.quantity
                    || row.gold_price_quantity.as_ref() == Some(&a.quantity)
                    || row.copper_price_quantity.as_ref() == Some(&a.quantity))
        })
        .cloned()
        .collect();
    canonical_readings(&mut out);
    out
}

fn compare(a: &IssuerReserveRow, b: &IssuerReserveRow) -> VintageComparison {
    let mut result = VintageComparison {
        left: a.declaration.id.clone(),
        right: b.declaration.id.clone(),
        status: "deck_unstated",
        overlap: None,
    };
    let (Some(left), Some(right)) = (&a.deck.key, &b.deck.key) else {
        return result;
    };
    if left != right {
        result.status = "deck_differs";
        return result;
    }
    let range = (|| -> Result<Interval, fmine_units::UnitsError> {
        let lower = a.interval.lo.checked_cmp(b.interval.lo)?;
        let upper = a.interval.hi.checked_cmp(b.interval.hi)?;
        let lo = if lower.is_gt() {
            a.interval.lo
        } else {
            b.interval.lo
        };
        let hi = if upper.is_lt() {
            a.interval.hi
        } else {
            b.interval.hi
        };
        let lo_strict = if lower.is_gt() {
            a.interval.lo_strict
        } else if lower.is_lt() {
            b.interval.lo_strict
        } else {
            a.interval.lo_strict || b.interval.lo_strict
        };
        let hi_strict = if upper.is_lt() {
            a.interval.hi_strict
        } else if upper.is_gt() {
            b.interval.hi_strict
        } else {
            a.interval.hi_strict || b.interval.hi_strict
        };
        Ok(Interval {
            lo,
            hi,
            lo_strict,
            hi_strict,
        })
    })();
    match range {
        Ok(range) => match range.lo.checked_cmp(range.hi) {
            Ok(order)
                if order.is_lt() || (order.is_eq() && !range.lo_strict && !range.hi_strict) =>
            {
                result.status = if a.source.cell.printed == b.source.cell.printed {
                    "unchanged"
                } else {
                    "rounding_compatible"
                };
                result.overlap = Some(range);
            }
            Ok(_) => result.status = "changed",
            Err(_) => result.status = "indeterminate",
        },
        Err(_) => result.status = "indeterminate",
    }
    result
}

/// Builds the retrospective register from the declared population. No unavailable newer filing is erased
/// before selecting the latest filing date. Source bytes and the completeness of reader results are checked
/// by the caller; this boundary rechecks each supplied accepted reading and its metadata.
#[must_use]
pub fn assess(
    registry: &TableIdentityRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> ReserveReport {
    let mut report = ReserveReport {
        issuer: registry.issuer.clone(),
        rows: Vec::new(),
        selections: Vec::new(),
        comparisons: Vec::new(),
        geo_checks: Vec::new(),
        refused: Vec::new(),
        registry_refusal: None,
        refused_input_digest: None,
    };
    if let Err(error) = registry.validate(documents) {
        report.registry_refusal = Some(error.to_string());
        report.refused_input_digest = Some(refused_input_digest(registry, readings, documents));
        return report;
    }
    let mut declarations: Vec<_> = registry.reserve_rows.iter().collect();
    declarations.sort_by(|a, b| a.id.cmp(&b.id));
    let mut groups: BTreeMap<ReserveKey, Vec<(&ReserveDeclaration, &DocumentRow)>> =
        BTreeMap::new();
    for row in declarations {
        let Some(document) = documents.iter().find(|d| d.file == row.doc) else {
            continue;
        };
        groups
            .entry(ReserveKey::of(&registry.issuer, row))
            .or_default()
            .push((row, document));
        match bind(&registry.issuer, row, readings, document) {
            Ok(bound) => report.rows.push(bound),
            Err(reason) => report.refused.push(ReserveRefusal {
                declaration: row.clone(),
                document: document.clone(),
                reason,
                evidence: evidence(row, readings),
            }),
        }
    }
    let usable: BTreeSet<_> = report
        .rows
        .iter()
        .map(|r| r.declaration.id.as_str())
        .collect();
    for (key, candidates) in groups {
        let filed: Vec<_> = candidates
            .iter()
            .filter(|(_, d)| matches!(d.kind.as_str(), "mda" | "aif"))
            .collect();
        let mut selection = CurrentSelection {
            economic_key: key.clone(),
            status: "no_filed_row",
            latest_filed_date: None,
            current_id: None,
            candidates: Vec::new(),
            unavailable: Vec::new(),
        };
        if filed.iter().any(|(_, d)| !is_date(&d.published_at)) {
            selection.status = "latest_unavailable";
            selection.candidates = filed.iter().map(|(r, _)| r.id.clone()).collect();
            selection.unavailable = selection.candidates.clone();
        } else if let Some(latest) = filed.iter().map(|(_, d)| &d.published_at).max() {
            selection.latest_filed_date = Some(latest.clone());
            selection.candidates = filed
                .iter()
                .filter(|(_, d)| &d.published_at == latest)
                .map(|(r, _)| r.id.clone())
                .collect();
            selection.unavailable = selection
                .candidates
                .iter()
                .filter(|id| !usable.contains(id.as_str()))
                .cloned()
                .collect();
            selection.status = if selection.candidates.len() > 1 {
                "ambiguous_latest"
            } else if !selection.unavailable.is_empty() {
                "latest_unavailable"
            } else {
                "current"
            };
            if selection.status == "current" {
                selection.current_id = selection.candidates.first().cloned();
            }
        }
        selection.candidates.sort();
        selection.unavailable.sort();
        report.selections.push(selection);
        let mut rows: Vec<_> = report
            .rows
            .iter()
            .filter(|r| r.economic_key == key)
            .collect();
        rows.sort_by(|a, b| {
            (&a.source.document.published_at, &a.declaration.id)
                .cmp(&(&b.source.document.published_at, &b.declaration.id))
        });
        for (i, a) in rows.iter().enumerate() {
            for b in &rows[i + 1..] {
                report.comparisons.push(compare(a, b));
            }
        }
    }
    report.geo_checks = geo::assess(registry, readings, documents);
    report
}

fn option(value: &Option<String>) -> String {
    value.as_ref().map_or_else(|| "null".into(), |s| js(s))
}
fn list(values: &[String]) -> String {
    format!(
        "[{}]",
        values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
    )
}

fn number(value: Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn declaration_json(d: &ReserveDeclaration) -> String {
    format!(
        "{{\"id\":{},\"doc\":{},\"quantity\":{},\"subject\":{},\"item\":{},\"reserve_year\":{},\"period\":{},\"metal\":{},\"unit\":{},\"ownership\":{},\"support\":{},\"convention\":{},\"deck\":{},\"deck_period\":{},\"deck_basis\":{},\"gold_price_quantity\":{},\"copper_price_quantity\":{}}}",
        js(&d.id),
        js(&d.doc),
        js(&d.quantity),
        js(&d.subject),
        js(&d.item),
        d.reserve_year,
        js(&d.period),
        js(&d.metal),
        js(&d.unit),
        js(&d.ownership),
        js(&d.support),
        js(&d.convention),
        js(&d.deck),
        option(&d.deck_period),
        option(&d.deck_basis),
        option(&d.gold_price_quantity),
        option(&d.copper_price_quantity)
    )
}

fn document_json(d: &DocumentRow) -> String {
    format!(
        "{{\"file\":{},\"dir\":{},\"kind\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"pages\":{},\"bytes\":{},\"url\":{},\"url_basis\":{}}}",
        js(&d.file),
        js(&d.dir),
        js(&d.kind),
        js(&d.sha256),
        js(&d.published_at),
        js(&d.published_at_basis),
        d.pages,
        d.bytes,
        js(&d.url),
        js(&d.url_basis)
    )
}

fn refused_input_digest(
    registry: &TableIdentityRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> String {
    // Sorting preserves duplicates: invalid repeated declarations or evidence must not disappear.
    let array = |mut rows: Vec<String>| {
        rows.sort();
        rows.join(",")
    };
    let identities = array(
        registry
            .identities
            .iter()
            .cloned()
            .map(|mut d| {
                d.terms.sort_by(|a, b| {
                    (&a.quantity, &a.period, a.sign, a.role).cmp(&(
                        &b.quantity,
                        &b.period,
                        b.sign,
                        b.role,
                    ))
                });
                definition_json(&d)
            })
            .collect(),
    );
    let reserves = array(registry.reserve_rows.iter().map(declaration_json).collect());
    let geo_checks = array(
        registry
            .geo_checks
            .iter()
            .map(geo::declaration_json)
            .collect(),
    );
    let cutoffs = array(
        registry
            .cutoffs
            .iter()
            .map(crate::cutoff::source::declaration_json)
            .collect(),
    );
    let documents = array(documents.iter().map(document_json).collect());
    let mut readings = readings.to_vec();
    canonical_readings(&mut readings);
    let readings = array(readings.iter().map(accepted_json).collect());
    sha256(
        format!(
            "{{\"schema\":\"fmine.reserve-refused-input.v1\",\"issuer\":{},\"identities\":[{identities}],\"reserve_rows\":[{reserves}],\"geo_checks\":[{geo_checks}],\"cutoffs\":[{cutoffs}],\"documents\":[{documents}],\"readings\":[{readings}]}}",
            js(&registry.issuer)
        )
        .as_bytes(),
    )
    .to_hex()
}

fn source_json(s: &SourceCell) -> String {
    format!(
        "{{\"reading\":{},\"printed\":{},\"step\":{},\"document\":{}}}",
        accepted_json(&s.source),
        rat(s.cell.printed),
        rat(s.cell.step),
        document_json(&s.document)
    )
}

impl IssuerReserveRow {
    #[must_use]
    pub const fn evidence_class(&self) -> EvidenceClass {
        EvidenceClass::ISSUER
    }
    #[must_use]
    pub fn declaration(&self) -> &ReserveDeclaration {
        &self.declaration
    }
    #[must_use]
    pub fn economic_key(&self) -> &ReserveKey {
        &self.economic_key
    }
    #[must_use]
    pub const fn source_role(&self) -> SourceRole {
        self.source_role
    }
    #[must_use]
    pub fn source(&self) -> &SourceCell {
        &self.source
    }
    #[must_use]
    pub const fn bounds(&self) -> Interval {
        self.interval
    }
    #[must_use]
    pub fn deck(&self) -> &ReserveDeck {
        &self.deck
    }
    fn body(&self) -> String {
        let price = |p: &Option<SourceCell>| p.as_ref().map_or_else(|| "null".into(), source_json);
        format!(
            "\"id\":{},\"doc\":{},\"published_at\":{},\"source_role\":{},\"evidence_class\":\"issuer_assertion\",\"declaration\":{},\"economic_key\":{},\"source\":{},\"interval\":{},\"deck\":{{\"status\":{},\"key\":{},\"period\":{},\"basis\":{},\"gold_price\":{},\"copper_price\":{}}}",
            js(&self.declaration.id),
            js(&self.declaration.doc),
            js(&self.source.document.published_at),
            js(self.source_role.label()),
            declaration_json(&self.declaration),
            self.economic_key.json(),
            source_json(&self.source),
            interval(&self.interval),
            js(if self.deck.key.is_some() {
                "quoted"
            } else {
                "unstated"
            }),
            option(&self.deck.key),
            option(&self.deck.period),
            option(&self.deck.basis),
            price(&self.deck.gold_price),
            price(&self.deck.copper_price)
        )
    }
    #[must_use]
    pub fn digest(&self) -> String {
        sha256(format!("{{{}}}", self.body()).as_bytes()).to_hex()
    }
    fn json(&self) -> String {
        format!("{{{},\"digest\":{}}}", self.body(), js(&self.digest()))
    }
}

impl ReserveReport {
    fn body(&self) -> String {
        let rows = self
            .rows
            .iter()
            .map(IssuerReserveRow::json)
            .collect::<Vec<_>>()
            .join(",");
        let selections = self.selections.iter().map(|s| format!("{{\"economic_key\":{},\"status\":{},\"latest_filed_date\":{},\"current_id\":{},\"candidates\":{},\"unavailable\":{}}}",s.economic_key.json(),js(s.status),option(&s.latest_filed_date),option(&s.current_id),list(&s.candidates),list(&s.unavailable))).collect::<Vec<_>>().join(",");
        let comparisons = self
            .comparisons
            .iter()
            .map(|c| {
                format!(
                    "{{\"left\":{},\"right\":{},\"status\":{},\"overlap\":{}}}",
                    js(&c.left),
                    js(&c.right),
                    js(c.status),
                    c.overlap.as_ref().map_or_else(|| "null".into(), interval)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let refused = self
            .refused
            .iter()
            .map(|r| {
                format!(
                    "{{\"id\":{},\"doc\":{},\"declaration\":{},\"document\":{},\"reason\":{},\"evidence\":[{}]}}",
                    js(&r.declaration.id),
                    js(&r.declaration.doc),
                    declaration_json(&r.declaration),
                    document_json(&r.document),
                    js(&r.reason),
                    r.evidence
                        .iter()
                        .map(accepted_json)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let geo_checks = self
            .geo_checks
            .iter()
            .map(geo::GeoCheckReport::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "\"schema\":\"fmine.issuer-reserves.v1\",\"issuer\":{},\"gate_eligible\":false,\"rows\":[{rows}],\"selections\":[{selections}],\"comparisons\":[{comparisons}],\"geo_checks\":[{geo_checks}],\"refused\":[{refused}],\"registry_refusal\":{},\"refused_input_digest\":{},\"no_claim\":{}",
            js(&self.issuer),
            option(&self.registry_refusal),
            option(&self.refused_input_digest),
            js(NO_CLAIM)
        )
    }
    #[must_use]
    pub fn digest(&self) -> String {
        sha256(format!("{{{}}}", self.body()).as_bytes()).to_hex()
    }
    #[must_use]
    pub fn to_json(&self) -> String {
        format!("{{{},\"digest\":{}}}", self.body(), js(&self.digest()))
    }
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let text = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('|', "&#124;")
                .replace('`', "&#96;")
                .replace('[', "&#91;")
                .replace(']', "&#93;")
                .replace('\\', "&#92;")
                .replace(['\n', '\r'], " ")
        };
        let mut out = format!(
            "\n## Issuer reserve assertions\n\n{NO_CLAIM}\n\nRegister digest: `{}`\n\n| Row / document | Publication / role | Subject / year / item | Printed assertion / unit / basis | Price deck |\n|---|---|---|---|---|\n",
            self.digest()
        );
        for row in &self.rows {
            let d = &row.declaration;
            let price = |p: &Option<SourceCell>, metal: &str| {
                p.as_ref().map_or_else(String::new, |p| {
                    format!("{metal}: {} ({})", p.source.token, p.source.locator)
                })
            };
            let deck = if row.deck.key.is_none() {
                "unstated".into()
            } else {
                format!(
                    "{}; {}",
                    price(&row.deck.gold_price, "gold"),
                    price(&row.deck.copper_price, "copper")
                )
            };
            let _ = writeln!(
                out,
                "| {} / {} | {} / {} | {} / {} / {} | {} {} / {} | {} |",
                text(&d.id),
                text(&d.doc),
                row.source.document.published_at,
                row.source_role.label(),
                text(&d.subject),
                d.reserve_year,
                text(&d.item),
                text(&row.source.source.token),
                text(&d.unit),
                text(&d.basis()),
                text(&deck)
            );
            let _ = writeln!(
                out,
                "\nSource: {}; SHA-256 `{}`. Printed rounding interval: {}{}, {}{} {}; convention: {}.\n",
                text(&row.source.source.locator.to_string()),
                row.source.source.doc_sha256,
                if row.interval.lo_strict { "(" } else { "[" },
                number(row.interval.lo),
                number(row.interval.hi),
                if row.interval.hi_strict { ")" } else { "]" },
                text(&d.unit),
                text(&d.convention)
            );
        }
        out.push_str("\n### Latest declared filings\n\n| Subject / year / item / basis | Status | Latest filing date | Current row | Candidates / unavailable |\n|---|---|---|---|---|\n");
        for s in &self.selections {
            let k = &s.economic_key;
            let _ = writeln!(
                out,
                "| {} / {} / {} / {} / {} / {} / {} | {} | {} | {} | {} / {} |",
                text(&k.subject),
                k.reserve_year,
                text(&k.item),
                text(&k.metal),
                text(&k.unit),
                text(&k.ownership),
                text(&k.support),
                s.status,
                s.latest_filed_date.as_deref().unwrap_or("—"),
                text(s.current_id.as_deref().unwrap_or("—")),
                text(&s.candidates.join(", ")),
                text(&s.unavailable.join(", "))
            );
        }
        if !self.comparisons.is_empty() {
            out.push_str("\n### Vintage and deck cross-checks\n\n| Earlier row | Later row | Comparison |\n|---|---|---|\n");
            for c in &self.comparisons {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    text(&c.left),
                    text(&c.right),
                    c.status
                );
            }
        }
        if let Some(reason) = &self.registry_refusal {
            let _ = writeln!(out, "\nRegistry refused: {}\n", text(reason));
        }
        for check in &self.geo_checks {
            out.push_str(&check.to_markdown());
        }
        for r in &self.refused {
            let _ = writeln!(
                out,
                "\nUnresolved reserve row {} ({}): {}\n",
                text(&r.declaration.id),
                text(&r.declaration.doc),
                text(&r.reason)
            );
            for a in &r.evidence {
                let _ = writeln!(
                    out,
                    "- {}: {}; SHA-256 `{}`; readers {}.",
                    text(&a.locator.to_string()),
                    text(&a.token),
                    text(&a.doc_sha256),
                    text(&a.readers.join(", "))
                );
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "../../xtask/testdata/reserves_unit_tests.rs"]
mod tests;
