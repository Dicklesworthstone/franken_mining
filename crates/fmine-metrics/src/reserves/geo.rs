//! Source-bound GEO checks at explicitly quoted price decks (BD-340.S4, BD-070).
//! Determinism: Exact (canonical sources and existing rational interval/discovery kernels).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_core::hash::sha256;
use fmine_numerics::exact_feas::Convention;
use fmine_numerics::exact_feas::discover::{
    Deck, DeckDiscovery, DeckFinding, Factor, Fit, GeoEvaluation, GeoRelation, discover_deck,
    evaluate_geo,
};
use fmine_registry::sources::DocumentRow;
use fmine_registry::table_identities::{
    GeoCheckDeclaration, ReserveDeclaration, TableIdentityRegistry,
};
use fmine_registry::transcriptions::Accepted;

use super::{
    ReserveDeck, bind_deck, declaration_json as reserve_json, document_json, role, source_json,
};
use crate::roll_forward::{
    SourceCell, accepted_json, bind_reading, canonical_readings, convention, interval, js,
};

pub const NO_CLAIM: &str = "Exact compatibility of cited issuer amounts with quoted price parameters. Alternative-deck discovery is limited to the explicitly supplied candidates and this check's amounts; it identifies neither the issuer's actual calculation nor a document-wide rounding convention. It changes no printed value, registered deck, current filing or reserve classification.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoAmount {
    pub role: &'static str,
    pub declaration: ReserveDeclaration,
    pub document: DocumentRow,
    pub source: Option<SourceCell>,
    pub refusal: Option<String>,
    pub evidence: Vec<Accepted>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoDeckInput {
    pub declaration: ReserveDeclaration,
    pub document: DocumentRow,
    pub primary: bool,
    pub deck: Option<ReserveDeck>,
    pub refusal: Option<String>,
    pub evidence: Vec<Accepted>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoCheckReport {
    pub declaration: GeoCheckDeclaration,
    pub doc: String,
    pub declared_convention: String,
    pub amounts: Vec<GeoAmount>,
    pub decks: Vec<GeoDeckInput>,
    pub declared: Option<(String, GeoEvaluation)>,
    pub discovery: Option<DeckDiscovery>,
    pub refusal: Option<String>,
}

fn selected(row: &ReserveDeclaration, readings: &[Accepted], prices: bool) -> Vec<Accepted> {
    let mut evidence: Vec<_> = readings
        .iter()
        .filter(|a| {
            a.locator.doc == row.doc
                && if prices {
                    Some(&a.quantity) == row.gold_price_quantity.as_ref()
                        || Some(&a.quantity) == row.copper_price_quantity.as_ref()
                } else {
                    a.quantity == row.quantity
                }
        })
        .cloned()
        .collect();
    canonical_readings(&mut evidence);
    evidence
}

fn deck_parameter(input: &GeoDeckInput) -> Option<Deck> {
    let deck = input.deck.as_ref()?;
    Some(Deck {
        name: deck.key.clone()?,
        gold: deck.gold_price.as_ref()?.cell.printed,
        copper: deck.copper_price.as_ref()?.cell.printed,
    })
}

pub(super) fn assess(
    registry: &TableIdentityRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> Vec<GeoCheckReport> {
    let mut definitions = registry.geo_checks.clone();
    definitions.sort_by(|a, b| a.id.cmp(&b.id));
    definitions
        .into_iter()
        .map(|mut definition| {
            definition.candidate_deck_rows.sort();
            let mut report = GeoCheckReport {
                declaration: definition,
                doc: String::new(),
                declared_convention: String::new(),
                amounts: Vec::new(),
                decks: Vec::new(),
                declared: None,
                discovery: None,
                refusal: None,
            };
            let bind_inputs = |report: &mut GeoCheckReport| -> Result<(), String> {
                let find = |id: &str| {
                    let row = registry
                        .reserve_rows
                        .iter()
                        .find(|r| r.id == id)
                        .ok_or_else(|| format!("unknown reserve row {id}"))?;
                    let document = documents
                        .iter()
                        .find(|d| d.file == row.doc)
                        .ok_or_else(|| format!("unregistered document {}", row.doc))?;
                    Ok::<_, String>((row, document))
                };
                for (name, id) in [
                    ("gold", &report.declaration.gold_row),
                    ("copper", &report.declaration.copper_row),
                    ("geo", &report.declaration.geo_row),
                ] {
                    let (row, document) = find(id)?;
                    if name == "geo" {
                        report.doc = row.doc.clone();
                        report.declared_convention = row.convention.clone();
                    }
                    let bound = role(document).and_then(|_| {
                        bind_reading(
                            &row.doc,
                            &row.quantity,
                            &row.period,
                            &row.unit,
                            &row.basis(),
                            readings,
                            document,
                        )
                    });
                    report.amounts.push(GeoAmount {
                        role: name,
                        declaration: row.clone(),
                        document: document.clone(),
                        source: bound.as_ref().ok().cloned(),
                        refusal: bound.err(),
                        evidence: selected(row, readings, false),
                    });
                }
                for id in std::iter::once(&report.declaration.geo_row)
                    .chain(&report.declaration.candidate_deck_rows)
                {
                    let (row, document) = find(id)?;
                    let bound = bind_deck(row, readings, document);
                    report.decks.push(GeoDeckInput {
                        declaration: row.clone(),
                        document: document.clone(),
                        primary: id == &report.declaration.geo_row,
                        deck: bound.as_ref().ok().cloned(),
                        refusal: bound.err(),
                        evidence: selected(row, readings, true),
                    });
                }
                Ok(())
            };
            if let Err(error) = bind_inputs(&mut report) {
                report.refusal = Some(error);
                return report;
            }
            let mut failures: Vec<_> = report
                .amounts
                .iter()
                .filter_map(|a| {
                    a.refusal
                        .as_ref()
                        .map(|e| format!("amount {}: {e}", a.declaration.id))
                })
                .chain(report.decks.iter().filter_map(|d| {
                    d.refusal
                        .as_ref()
                        .map(|e| format!("deck {}: {e}", d.declaration.id))
                }))
                .collect();
            let factors: Vec<_> = report
                .amounts
                .iter()
                .filter_map(|a| a.source.as_ref())
                .map(|s| Factor {
                    printed: s.cell.printed,
                    step: s.cell.step,
                })
                .collect();
            if let [gold, copper, geo] = factors.as_slice() {
                let relation = GeoRelation {
                    gold: *gold,
                    copper: *copper,
                    geo: *geo,
                };
                if let Some(primary) = report.decks.first().and_then(deck_parameter) {
                    match evaluate_geo(&relation, &primary, convention(&report.declared_convention))
                    {
                        Ok(evaluation) => report.declared = Some((primary.name, evaluation)),
                        Err(error) => failures
                            .push(format!("declared-deck evaluation unavailable: {error:?}")),
                    }
                }
                if failures.is_empty() {
                    // Equal parameter keys are one hypothesis, while every source provider remains above.
                    let parameters: BTreeMap<_, _> = report
                        .decks
                        .iter()
                        .filter_map(deck_parameter)
                        .map(|d| (d.name.clone(), d))
                        .collect();
                    match discover_deck(&[relation], &parameters.into_values().collect::<Vec<_>>())
                    {
                        Ok(discovery) => report.discovery = Some(discovery),
                        Err(error) => {
                            failures.push(format!("deck discovery unavailable: {error:?}"))
                        }
                    }
                }
            }
            failures.sort();
            if !failures.is_empty() {
                report.refusal = Some(failures.join("; "));
            }
            report
        })
        .collect()
}

pub(super) fn declaration_json(d: &GeoCheckDeclaration) -> String {
    let mut candidates = d.candidate_deck_rows.clone();
    candidates.sort();
    format!(
        "{{\"id\":{},\"gold_row\":{},\"copper_row\":{},\"geo_row\":{},\"candidate_deck_rows\":{}}}",
        js(&d.id),
        js(&d.gold_row),
        js(&d.copper_row),
        js(&d.geo_row),
        super::list(&candidates)
    )
}

fn fit(fit: Fit) -> &'static str {
    match fit {
        Fit::Empty => "empty",
        Fit::Points => "points",
        Fit::Proper => "proper",
    }
}

fn convention_name(convention: Convention) -> &'static str {
    match convention {
        Convention::Closed => "closed",
        Convention::HalfUp => "half_up",
        Convention::HalfDown => "half_down",
        Convention::HalfEven => "half_even",
        Convention::Truncate => "truncate",
    }
}

fn evidence_json(rows: &[Accepted]) -> String {
    format!(
        "[{}]",
        rows.iter().map(accepted_json).collect::<Vec<_>>().join(",")
    )
}

impl GeoCheckReport {
    fn body(&self) -> String {
        let amounts = self.amounts.iter().map(|a| format!(
            "{{\"role\":{},\"row\":{},\"declaration\":{},\"document\":{},\"source\":{},\"refusal\":{},\"evidence\":{}}}",
            js(a.role), js(&a.declaration.id), reserve_json(&a.declaration), document_json(&a.document),
            a.source.as_ref().map_or_else(|| "null".into(), source_json), super::option(&a.refusal), evidence_json(&a.evidence)
        )).collect::<Vec<_>>().join(",");
        let decks = self.decks.iter().map(|d| {
            let price = |gold: bool| d.deck.as_ref().and_then(|deck| if gold { deck.gold_price.as_ref() } else { deck.copper_price.as_ref() })
                .map_or_else(|| "null".into(), source_json);
            format!("{{\"row\":{},\"primary\":{},\"declaration\":{},\"document\":{},\"key\":{},\"period\":{},\"basis\":{},\"gold_price\":{},\"copper_price\":{},\"refusal\":{},\"evidence\":{}}}",
                js(&d.declaration.id), d.primary, reserve_json(&d.declaration), document_json(&d.document),
                super::option(&d.deck.as_ref().and_then(|deck| deck.key.clone())),
                super::option(&d.declaration.deck_period), super::option(&d.declaration.deck_basis),
                price(true), price(false), super::option(&d.refusal), evidence_json(&d.evidence))
        }).collect::<Vec<_>>().join(",");
        let declared = self.declared.as_ref().map_or_else(
            || "null".into(),
            |(key, result)| {
                format!(
                    "{{\"deck_key\":{},\"fit\":{},\"converted\":{},\"reported\":{}}}",
                    js(key),
                    js(fit(result.fit)),
                    interval(&result.converted),
                    interval(&result.reported)
                )
            },
        );
        let discovery = self.discovery.as_ref().map_or_else(
            || "null".into(),
            |d| {
                let (finding, keys) = match &d.finding {
                    DeckFinding::Found(key) => ("found", vec![key.clone()]),
                    DeckFinding::Ambiguous(keys) => ("ambiguous", keys.clone()),
                    DeckFinding::NoFit => ("no_fit", Vec::new()),
                };
                let fits = d
                    .fits
                    .iter()
                    .map(|f| {
                        format!(
                            "{{\"deck_key\":{},\"convention\":{},\"fit\":{}}}",
                            js(&f.deck),
                            js(convention_name(f.convention)),
                            js(fit(f.fit))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let conventions: Vec<_> = d
                    .conventions
                    .iter()
                    .map(|c| convention_name(*c).to_string())
                    .collect();
                format!(
                    "{{\"finding\":{},\"deck_keys\":{},\"conventions\":{},\"fits\":[{fits}]}}",
                    js(finding),
                    super::list(&keys),
                    super::list(&conventions)
                )
            },
        );
        format!(
            "\"schema\":\"fmine.geo-check.v1\",\"id\":{},\"doc\":{},\"declaration\":{},\"status\":{},\"declared_convention\":{},\"declared\":{declared},\"discovery\":{discovery},\"amounts\":[{amounts}],\"decks\":[{decks}],\"refusal\":{},\"no_claim\":{}",
            js(&self.declaration.id),
            js(&self.doc),
            declaration_json(&self.declaration),
            js(if self.refusal.is_some() {
                "unavailable"
            } else {
                "checked"
            }),
            js(&self.declared_convention),
            super::option(&self.refusal),
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
            "\n### GEO conversion {}\n\n{NO_CLAIM}\n\nCheck digest: `{}`\n",
            text(&self.declaration.id),
            self.digest()
        );
        for amount in &self.amounts {
            if let Some(s) = &amount.source {
                let _ = writeln!(
                    out,
                    "\n{}: {} {}; {}; SHA-256 `{}`.",
                    amount.role,
                    text(&s.source.token),
                    text(&s.source.unit),
                    text(&s.source.locator.to_string()),
                    s.source.doc_sha256
                );
            }
        }
        if let Some((_, result)) = &self.declared {
            let bounds = |i: &fmine_numerics::exact_feas::Interval| {
                format!(
                    "{}{}, {}{}",
                    if i.lo_strict { "(" } else { "[" },
                    super::number(i.lo),
                    super::number(i.hi),
                    if i.hi_strict { ")" } else { "]" }
                )
            };
            let _ = writeln!(
                out,
                "\nStated deck under {}: **{}**. Converted GEO interval {} Moz; reported interval {} Moz.\n",
                text(&self.declared_convention),
                fit(result.fit),
                bounds(&result.converted),
                bounds(&result.reported)
            );
        }
        out.push_str("\n| Price source row | Role / parameter key | Gold and copper prices |\n|---|---|---|\n");
        for input in &self.decks {
            let parameters = input.deck.as_ref().map_or_else(
                || "unavailable".into(),
                |d| {
                    [&d.gold_price, &d.copper_price]
                        .into_iter()
                        .flatten()
                        .map(|s| {
                            format!(
                                "{} {} at {}; SHA-256 {}",
                                s.source.token,
                                s.source.unit,
                                s.source.locator,
                                s.source.doc_sha256
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                },
            );
            let key = input
                .deck
                .as_ref()
                .and_then(|d| d.key.as_deref())
                .unwrap_or("unavailable");
            let _ = writeln!(
                out,
                "| {} | {} / {} | {} |",
                text(&input.declaration.id),
                if input.primary { "stated" } else { "candidate" },
                text(key),
                text(&parameters)
            );
        }
        if let Some(discovery) = &self.discovery {
            let finding = match &discovery.finding {
                DeckFinding::Found(key) => format!(
                    "one compatible parameter key among the declared candidates: {}",
                    text(key)
                ),
                DeckFinding::Ambiguous(keys) => {
                    format!("ambiguous across {} declared parameter keys", keys.len())
                }
                DeckFinding::NoFit => "no proper fit under any strict convention".into(),
            };
            let _ = writeln!(
                out,
                "\nAlternative-deck discovery: {finding}.\n\n| Parameter key | Convention | Fit |\n|---|---|---|"
            );
            for row in &discovery.fits {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    text(&row.deck),
                    convention_name(row.convention),
                    fit(row.fit)
                );
            }
        }
        if let Some(reason) = &self.refusal {
            let _ = writeln!(out, "\nGEO check unavailable: {}.\n", text(reason));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_registry::transcriptions::{Locator, parse_token};

    fn fixture() -> (TableIdentityRegistry, Vec<Accepted>, Vec<DocumentRow>) {
        let documents: Vec<_> = ["Report.pdf", "Alternative.pdf"]
            .into_iter()
            .enumerate()
            .map(|(i, file)| DocumentRow {
                file: file.into(),
                dir: "synthetic".into(),
                kind: "mda".into(),
                url: format!("https://example.test/{file}"),
                url_basis: "synthetic".into(),
                published_at: format!("203{}-02-01", i + 1),
                published_at_basis: "FACT: invented fixture".into(),
                pages: 2,
                bytes: 100,
                sha256: if i == 0 { "a" } else { "b" }.repeat(64),
            })
            .collect();
        let mut rows = Vec::new();
        let mut readings = Vec::new();
        let mut add = |doc: &DocumentRow,
                       quantity: &str,
                       token: &str,
                       unit: &str,
                       basis: &str,
                       period: &str| {
            readings.push(Accepted {
                locator: Locator {
                    doc: doc.file.clone(),
                    page: 1,
                    table: "Invented GEO".into(),
                    row: quantity.into(),
                    col: period.into(),
                },
                doc_sha256: doc.sha256.clone(),
                token: token.into(),
                value: parse_token(token).unwrap(),
                quantity: quantity.into(),
                unit: unit.into(),
                basis: basis.into(),
                period: period.into(),
                readers: vec!["reader_a".into(), "reader_b".into()],
            });
        };
        for (id, metal, unit, token, doc_index) in [
            ("gold", "gold", "moz", "10.0", 0),
            ("copper", "copper", "mlb", "1000", 0),
            ("geo", "geo", "moz", "12.0", 0),
            ("alternative", "geo", "moz", "99.0", 1),
        ] {
            let quoted = metal == "geo";
            let year = 2030 + doc_index as u16;
            let row = ReserveDeclaration {
                id: id.into(),
                doc: documents[doc_index].file.clone(),
                quantity: id.into(),
                subject: "invented_issuer".into(),
                item: "net_change".into(),
                reserve_year: year,
                period: format!("{year}FY"),
                metal: metal.into(),
                unit: unit.into(),
                ownership: "full".into(),
                support: "contained".into(),
                convention: "closed".into(),
                deck: if quoted { "quoted" } else { "unstated" }.into(),
                deck_period: quoted.then(|| format!("{year}FY")),
                deck_basis: quoted.then(|| "reserve_prices".into()),
                gold_price_quantity: quoted.then(|| "gold_price".into()),
                copper_price_quantity: quoted.then(|| "copper_price".into()),
            };
            add(
                &documents[doc_index],
                id,
                token,
                unit,
                &row.basis(),
                &row.period,
            );
            if quoted {
                add(
                    &documents[doc_index],
                    "gold_price",
                    "1500",
                    "usd_per_oz",
                    "reserve_prices",
                    &row.period,
                );
                add(
                    &documents[doc_index],
                    "copper_price",
                    if doc_index == 0 { "3.00" } else { "4.50" },
                    "usd_per_lb",
                    "reserve_prices",
                    &row.period,
                );
            }
            rows.push(row);
        }
        (
            TableIdentityRegistry {
                issuer: "synthetic".into(),
                identities: Vec::new(),
                reserve_rows: rows,
                geo_checks: vec![GeoCheckDeclaration {
                    id: "conversion".into(),
                    gold_row: "gold".into(),
                    copper_row: "copper".into(),
                    geo_row: "geo".into(),
                    candidate_deck_rows: vec!["alternative".into()],
                }],
                cutoffs: Vec::new(),
            },
            readings,
            documents,
        )
    }

    fn token(readings: &mut [Accepted], doc: &str, quantity: &str, token: &str) {
        let reading = readings
            .iter_mut()
            .find(|a| a.locator.doc == doc && a.quantity == quantity)
            .unwrap();
        reading.token = token.into();
        reading.value = parse_token(token).unwrap();
    }

    #[test]
    fn cited_conversion_and_alternative_deck_findings_do_not_replace_the_stated_deck() {
        let (registry, mut readings, documents) = fixture();
        let report = super::super::assess(&registry, &readings, &documents);
        let checked = &report.geo_checks[0];
        assert!(checked.refusal.is_none());
        assert_eq!(checked.declared.as_ref().unwrap().1.fit, Fit::Proper);
        let stated = checked.declared.as_ref().unwrap().0.clone();
        assert_eq!(
            checked.discovery.as_ref().unwrap().finding,
            DeckFinding::Found(stated.clone())
        );
        token(&mut readings, "Report.pdf", "geo", "13.0");
        let report = super::super::assess(&registry, &readings, &documents);
        let checked = &report.geo_checks[0];
        assert_eq!(checked.declared.as_ref().unwrap().0, stated);
        assert_eq!(checked.declared.as_ref().unwrap().1.fit, Fit::Empty);
        assert_eq!(
            checked.discovery.as_ref().unwrap().finding,
            DeckFinding::Found(checked.decks[1].deck.as_ref().unwrap().key.clone().unwrap())
        );
        assert_eq!(
            checked.amounts[2].source.as_ref().unwrap().source.token,
            "13.0"
        );
        assert!(checked.to_json().contains("issuer's actual calculation"));
    }

    #[test]
    fn equal_ratios_remain_ambiguous_and_identical_parameters_keep_every_source_provider() {
        let (mut registry, mut readings, documents) = fixture();
        token(&mut readings, "Alternative.pdf", "gold_price", "3000");
        token(&mut readings, "Alternative.pdf", "copper_price", "6.00");
        let report = super::super::assess(&registry, &readings, &documents);
        assert!(
            matches!(&report.geo_checks[0].discovery.as_ref().unwrap().finding, DeckFinding::Ambiguous(keys) if keys.len() == 2)
        );
        token(&mut readings, "Alternative.pdf", "gold_price", "1500");
        token(&mut readings, "Alternative.pdf", "copper_price", "3.00");
        registry.reserve_rows[3].deck_period = Some("2030FY".into());
        for reading in &mut readings {
            if reading.locator.doc == "Alternative.pdf" && reading.quantity.ends_with("price") {
                reading.period = "2030FY".into();
            }
        }
        let report = super::super::assess(&registry, &readings, &documents);
        let checked = &report.geo_checks[0];
        assert_eq!(checked.decks.len(), 2);
        assert_eq!(
            checked.decks[0].deck.as_ref().unwrap().key,
            checked.decks[1].deck.as_ref().unwrap().key
        );
        assert_eq!(checked.discovery.as_ref().unwrap().fits.len(), 4);
        assert_ne!(
            checked.decks[0].document.sha256,
            checked.decks[1].document.sha256
        );
        assert!(checked.to_markdown().contains("Alternative.pdf"));
    }

    #[test]
    fn candidate_price_availability_is_independent_of_its_unrelated_reserve_amount() {
        let (registry, readings, documents) = fixture();
        let amounts_missing: Vec<_> = readings
            .iter()
            .filter(|a| a.quantity != "alternative")
            .cloned()
            .collect();
        let report = super::super::assess(&registry, &amounts_missing, &documents);
        assert!(
            report
                .refused
                .iter()
                .any(|r| r.declaration.id == "alternative")
        );
        assert!(report.geo_checks[0].refusal.is_none());
        assert!(report.geo_checks[0].discovery.is_some());
        let prices_missing: Vec<_> = readings
            .iter()
            .filter(|a| !(a.locator.doc == "Alternative.pdf" && a.quantity == "copper_price"))
            .cloned()
            .collect();
        let report = super::super::assess(&registry, &prices_missing, &documents);
        let checked = &report.geo_checks[0];
        assert_eq!(checked.decks.len(), 2);
        assert!(checked.declared.is_some());
        assert!(checked.discovery.is_none());
        assert!(checked.refusal.as_ref().unwrap().contains("alternative"));
        assert!(!checked.decks[1].evidence.is_empty());
    }

    #[test]
    fn missing_or_unusable_amounts_and_prices_never_create_a_unique_finding() {
        let (registry, readings, documents) = fixture();
        for mutation in 0..6 {
            let mut broken = readings.clone();
            match mutation {
                0 => broken.retain(|a| a.quantity != "gold"),
                1 => broken
                    .iter_mut()
                    .find(|a| a.quantity == "copper")
                    .unwrap()
                    .readers
                    .pop()
                    .map(|_| ())
                    .unwrap(),
                2 => {
                    broken
                        .iter_mut()
                        .find(|a| a.locator.doc == "Alternative.pdf" && a.quantity == "gold_price")
                        .unwrap()
                        .unit = "usd_per_lb".into()
                }
                3 => {
                    broken
                        .iter_mut()
                        .find(|a| a.locator.doc == "Alternative.pdf" && a.quantity == "gold_price")
                        .unwrap()
                        .period = "2029FY".into()
                }
                4 => token(&mut broken, "Report.pdf", "gold_price", "0"),
                _ => {
                    let mut competitor = broken
                        .iter()
                        .find(|a| a.quantity == "copper")
                        .unwrap()
                        .clone();
                    competitor.locator.row = "competing copper".into();
                    broken.push(competitor);
                }
            }
            let report = super::super::assess(&registry, &broken, &documents);
            let checked = &report.geo_checks[0];
            assert!(checked.refusal.is_some(), "mutation {mutation}");
            assert!(checked.discovery.is_none(), "mutation {mutation}");
        }
    }

    #[test]
    fn alternate_convention_does_not_override_the_declared_document_convention() {
        let (mut registry, mut readings, documents) = fixture();
        registry.geo_checks[0].candidate_deck_rows.clear();
        token(&mut readings, "Report.pdf", "geo", "20.06");
        token(&mut readings, "Report.pdf", "gold_price", "1000");
        token(&mut readings, "Report.pdf", "copper_price", "10");
        let closed = super::super::assess(&registry, &readings, &documents);
        assert_eq!(
            closed.geo_checks[0].declared.as_ref().unwrap().1.fit,
            Fit::Points
        );
        for row in &mut registry.reserve_rows[..3] {
            row.convention = "half_up".into();
        }
        let open = super::super::assess(&registry, &readings, &documents);
        let checked = &open.geo_checks[0];
        assert_eq!(checked.declared.as_ref().unwrap().1.fit, Fit::Empty);
        assert!(
            checked
                .discovery
                .as_ref()
                .unwrap()
                .conventions
                .contains(&Convention::Truncate)
        );
        assert_eq!(checked.declared_convention, "half_up");
    }

    #[test]
    fn geo_checks_are_order_invariant_and_bind_candidate_evidence_and_membership() {
        let (mut registry, mut readings, mut documents) = fixture();
        let before = super::super::assess(&registry, &readings, &documents);
        registry.reserve_rows.reverse();
        readings.reverse();
        documents.reverse();
        for reading in &mut readings {
            reading.readers.reverse();
        }
        assert_eq!(
            before.to_json(),
            super::super::assess(&registry, &readings, &documents).to_json()
        );
        documents
            .iter_mut()
            .find(|d| d.file == "Alternative.pdf")
            .unwrap()
            .url_basis = "changed citation metadata".into();
        let changed = super::super::assess(&registry, &readings, &documents);
        assert_ne!(
            before.geo_checks[0].digest(),
            changed.geo_checks[0].digest()
        );
        registry.geo_checks[0]
            .candidate_deck_rows
            .push("unknown".into());
        let invalid = super::super::assess(&registry, &readings, &documents);
        assert!(invalid.registry_refusal.is_some());
        registry.geo_checks[0]
            .candidate_deck_rows
            .push("another_unknown".into());
        assert_ne!(
            invalid.digest(),
            super::super::assess(&registry, &readings, &documents).digest()
        );
    }
}
