//! Source-bound technical-report cut-off reproduction (BD-274).
//! Determinism: DeterministicPerIsa (exact bounds; the inherited TCM solver cross-check uses floats).
//!
//! The caller authenticates document bytes and the complete reader agreement. This boundary rechecks
//! each accepted point, its identity and source metadata, and constructs the existing cut-off kernel's
//! inputs without reading numerical values from recipe metadata. Gold price is a stated exact parameter;
//! other inputs use their closed half-last-place boxes, and the published cut-off is rounded half up.

use std::fmt::Write as _;

use fmine_core::hash::sha256;
use fmine_registry::sources::DocumentRow;
use fmine_registry::table_identities::cutoff::{CutoffDeclaration, InputRole, RecoveryModel, Unit};
use fmine_registry::transcriptions::Accepted;
use fmine_units::ExactQty;

use super::{Corner, CutoffCase, CutoffReport, GradeUnit, Printed, Recovery, reproduce};
use crate::roll_forward::{SourceCell, accepted_json, bind_reading, canonical_readings, js};

mod claims;
pub use claims::{CutoffClaims, InputValue};

pub const NO_CLAIM: &str = "Arithmetic reproduction of declared technical-report assumptions, not an optimal operating cutoff, a resource or reserve classification, or evidence that the assumptions are physically true. Source bytes and complete independent reader agreement are authenticated by the caller. Rounded-input bounds are arithmetic enclosures, not probability intervals.";
pub const ROUNDING_RULE: &str = "closed half-last-place input boxes; exact stated gold-price parameter; half-up published cutoff";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub declaration: CutoffDeclaration,
    pub documents: Vec<DocumentRow>,
    pub evidence: Vec<Accepted>,
    pub result: Result<CutoffReport, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceReport {
    pub rows: Vec<SourceRow>,
}

fn exact(value: fmine_numerics::exact_feas::Rat) -> Result<ExactQty, String> {
    ExactQty::new(value.num(), value.den()).map_err(|e| format!("exact source value: {e:?}"))
}

fn printed(source: &SourceCell) -> Result<Printed, String> {
    Ok(Printed {
        value: exact(source.cell.printed)?,
        half_step: exact(source.cell.step)?
            .checked_div(ExactQty::from_integer(2))
            .map_err(|e| format!("exact source precision: {e:?}"))?,
    })
}

fn domain(case: &CutoffCase) -> Result<(), String> {
    let checked = (|| -> Result<(), super::CutoffFailure> {
        if case.published.value.signum() <= 0
            || case.published_metric.is_some_and(|p| p.value.signum() <= 0)
            || case.gold_price_usd_per_oz.signum() <= 0
        {
            return Err(super::CutoffFailure::Registry(
                "published grades and gold price must be positive".into(),
            ));
        }
        if case
            .costs_usd_per_t
            .iter()
            .chain(&case.revenue_percent)
            .chain(&case.usd_per_oz)
            .any(|p| p.value.signum() < 0)
            || super::sum(&case.costs_usd_per_t, Corner::Low)?.signum() <= 0
        {
            return Err(super::CutoffFailure::Registry("cost and deduction tokens must be nonnegative and the total cost's whole rounding box must be positive".into()));
        }
        let hundred = ExactQty::from_integer(100);
        let (low, high) = match case.recovery {
            Recovery::Percent(p) => (p.low()?, p.high()?),
            Recovery::TcmOverAu {
                intercept,
                slope,
                tcm_percent,
            } => {
                if intercept.low()?.signum() <= 0
                    || slope.low()?.signum() < 0
                    || tcm_percent.low()?.signum() < 0
                    || tcm_percent.high()?.checked_cmp(hundred)?.is_gt()
                {
                    return Err(super::CutoffFailure::Registry("TCM model needs a positive intercept, nonnegative slope, and TCM in [0,100] percent throughout the input boxes".into()));
                }
                // At the fixed point, R% = intercept * c / (c + slope*TCM/100), where c is
                // the no-recovery break-even grade. These exact extrema retain that dependence.
                let c = |corner| -> Result<ExactQty, super::CutoffFailure> {
                    let net = super::net_per_oz(case, corner)?;
                    if net.signum() <= 0 {
                        return Err(super::CutoffFailure::NoPositiveNet {
                            id: case.id.clone(),
                        });
                    }
                    let grams = ExactQty::new(
                        fmine_units::TROY_OZ_TO_GRAMS_NUM,
                        fmine_units::TROY_OZ_TO_GRAMS_DEN,
                    )?;
                    Ok(super::sum(&case.costs_usd_per_t, corner)?
                        .checked_mul(grams)?
                        .checked_div(net)?)
                };
                let (c_low, c_high) = (c(Corner::Low)?, c(Corner::High)?);
                let loss_low = slope
                    .low()?
                    .checked_mul(tcm_percent.low()?)?
                    .checked_div(hundred)?;
                let loss_high = slope
                    .high()?
                    .checked_mul(tcm_percent.high()?)?
                    .checked_div(hundred)?;
                (
                    intercept
                        .low()?
                        .checked_mul(c_low)?
                        .checked_div(c_low.checked_add(loss_high)?)?,
                    intercept
                        .high()?
                        .checked_mul(c_high)?
                        .checked_div(c_high.checked_add(loss_low)?)?,
                )
            }
        };
        if low.signum() <= 0 || high.checked_cmp(hundred)?.is_gt() {
            return Err(super::CutoffFailure::Registry("effective recovery must stay within (0,100] percent throughout the declared rounding boxes".into()));
        }
        Ok(())
    })();
    checked.map_err(|e| e.to_string())
}

fn bind(
    declaration: &CutoffDeclaration,
    evidence: &[Accepted],
    documents: &[DocumentRow],
) -> Result<(CutoffReport, Vec<(InputRole, SourceCell)>), String> {
    declaration.validate(documents).map_err(|e| e.to_string())?;
    let [document] = documents else {
        return Err("cutoff needs exactly one registered source document".into());
    };
    if document.kind != "technical_report" {
        return Err(format!(
            "{} has kind {}; cutoff inputs require technical_report",
            document.file, document.kind
        ));
    }
    let mut sources = Vec::new();
    for input in &declaration.inputs {
        let basis = if input.role == InputRole::GoldPrice {
            &declaration.price_basis
        } else {
            &declaration.basis
        };
        sources.push((
            input.role,
            bind_reading(
                &declaration.doc,
                &input.quantity,
                &declaration.period,
                input.role.unit(declaration.unit),
                basis,
                evidence,
                document,
            )?,
        ));
    }
    let one = |role| -> Result<&SourceCell, String> {
        sources
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, s)| s)
            .ok_or_else(|| format!("missing required role {}", role.as_str()))
    };
    let values = |role| -> Result<Vec<Printed>, String> {
        sources
            .iter()
            .filter(|(r, _)| *r == role)
            .map(|(_, s)| printed(s))
            .collect()
    };
    let published = one(InputRole::Published)?;
    let case = CutoffCase {
        id: declaration.id.clone(),
        document: document.file.clone(),
        page: i64::try_from(published.source.locator.page).map_err(|_| "source page overflow")?,
        table: published.source.locator.table.clone(),
        evidence: "registered two-reader technical-report inputs".into(),
        unit: match declaration.unit {
            Unit::GramsPerTonne => GradeUnit::GramsPerTonne,
            Unit::OuncesPerShortTon => GradeUnit::OuncesPerShortTon,
        },
        published: printed(published)?,
        published_metric: sources
            .iter()
            .find(|(r, _)| *r == InputRole::PublishedMetric)
            .map(|(_, s)| printed(s))
            .transpose()?,
        gold_price_usd_per_oz: exact(one(InputRole::GoldPrice)?.cell.printed)?,
        costs_usd_per_t: values(InputRole::Cost)?,
        revenue_percent: values(InputRole::RevenueDeduction)?,
        usd_per_oz: values(InputRole::OunceDeduction)?,
        recovery: match declaration.recovery {
            RecoveryModel::ConstantPercent => {
                Recovery::Percent(printed(one(InputRole::Recovery)?)?)
            }
            RecoveryModel::TcmOverGold => Recovery::TcmOverAu {
                intercept: printed(one(InputRole::RecoveryIntercept)?)?,
                slope: printed(one(InputRole::RecoverySlope)?)?,
                tcm_percent: printed(one(InputRole::Tcm)?)?,
            },
        },
    };
    domain(&case)?;
    Ok((reproduce(&case).map_err(|e| e.to_string())?, sources))
}

/// Every declared cutoff stays in the population, including malformed or unavailable source inputs.
#[must_use]
pub fn assess(
    declarations: &[CutoffDeclaration],
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> SourceReport {
    let mut rows = Vec::new();
    for declaration in declarations {
        let mut declaration = declaration.clone();
        declaration.inputs.sort();
        let mut evidence: Vec<_> = readings
            .iter()
            .filter(|r| {
                r.locator.doc == declaration.doc
                    && declaration.inputs.iter().any(|i| i.quantity == r.quantity)
            })
            .cloned()
            .collect();
        canonical_readings(&mut evidence);
        let mut documents: Vec<_> = documents
            .iter()
            .filter(|d| d.file == declaration.doc)
            .cloned()
            .collect();
        documents.sort_by_key(document_json);
        let result = if declarations
            .iter()
            .filter(|d| d.id == declaration.id)
            .count()
            != 1
        {
            Err("cutoff id is ambiguous".into())
        } else {
            bind(&declaration, &evidence, &documents).map(|(report, _)| report)
        };
        rows.push(SourceRow {
            declaration,
            documents,
            evidence,
            result,
        });
    }
    rows.sort_by_key(|row| declaration_json(&row.declaration));
    SourceReport { rows }
}

/// Canonical declaration bytes also bind registry-wide refusals in other projections.
pub(crate) fn declaration_json(d: &CutoffDeclaration) -> String {
    let mut inputs = d.inputs.clone();
    inputs.sort();
    let inputs = inputs
        .iter()
        .map(|i| {
            format!(
                "{{\"role\":{},\"quantity\":{}}}",
                js(i.role.as_str()),
                js(&i.quantity)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"doc\":{},\"subject\":{},\"period\":{},\"basis\":{},\"price_basis\":{},\"unit\":{},\"recovery\":{},\"inputs\":[{inputs}]}}",
        js(&d.id),
        js(&d.doc),
        js(&d.subject),
        js(&d.period),
        js(&d.basis),
        js(&d.price_basis),
        js(d.unit.as_str()),
        js(d.recovery.as_str())
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

fn ratio(value: ExactQty) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}

fn result_json(r: &CutoffReport) -> Result<String, String> {
    let fixed = |v| super::fixed(v, 6).map_err(|e| format!("result formatting: {e:?}"));
    Ok(format!(
        "{{\"id\":{},\"document\":{},\"page\":{},\"table\":{},\"unit\":{},\"published\":{},\"computed\":{},\"low\":{},\"high\":{},\"state\":{},\"solver_iterations\":{},\"metric_from_rounded\":{},\"exact\":{{\"published\":{},\"computed\":{},\"low\":{},\"high\":{}}}}}",
        js(&r.id),
        js(&r.document),
        r.page,
        js(&r.table),
        js(r.unit.symbol()),
        js(&fixed(r.published.value)?),
        js(&fixed(r.computed)?),
        js(&fixed(r.low)?),
        js(&fixed(r.high)?),
        js(r.state.as_str()),
        r.solver_iterations
            .map_or_else(|| "null".into(), |v| v.to_string()),
        r.metric_from_rounded
            .map_or_else(|| "null".into(), |v| v.to_string()),
        ratio(r.published.value),
        ratio(r.computed),
        ratio(r.low),
        ratio(r.high)
    ))
}

impl SourceReport {
    pub(crate) fn refuse_all(&mut self, reason: &str) {
        for row in &mut self.rows {
            row.result = Err(reason.into());
        }
    }

    fn body(&self) -> String {
        let rows = self.rows.iter().map(|row| {
            let rendered = row.claims().and_then(|claims| {
                Ok((result_json(&claims.derived.value)?, claims.to_json()?))
            });
            let (status, result, claims, refusal) = match (&row.result, rendered) {
                (Ok(report), Ok((json, claims))) => (report.state.as_str(), json, claims, "null".into()),
                (_, Err(error)) => ("refused", "null".into(), "null".into(), js(&error)),
                (Err(error), _) => ("refused", "null".into(), "null".into(), js(error)),
            };
            let documents = row.documents.iter().map(document_json).collect::<Vec<_>>().join(",");
            let evidence = row.evidence.iter().map(accepted_json).collect::<Vec<_>>().join(",");
            format!("{{\"id\":{},\"declaration\":{},\"documents\":[{documents}],\"evidence\":[{evidence}],\"status\":{},\"result\":{result},\"refusal\":{refusal},\"claims\":{claims}}}",
                js(&row.declaration.id), declaration_json(&row.declaration), js(status))
        }).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"fmine.source-cutoffs.v1\",\"gate_eligible\":false,\"rounding_rule\":{},\"rows\":[{rows}],\"no_claim\":{}",
            js(ROUNDING_RULE),
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
        if self.rows.is_empty() {
            return String::new();
        }
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
            "\n## Source-bound published cut-offs\n\n{NO_CLAIM}\n\nRounding rule: {ROUNDING_RULE}.\n\nDigest: `{}`\n",
            self.digest()
        );
        for row in &self.rows {
            let d = &row.declaration;
            let _ = write!(
                out,
                "\n### {} — {}\n\n{}; {}; physical/model basis {}; price basis {}.\n\n",
                text(&d.id),
                text(&d.subject),
                text(&d.doc),
                text(&d.period),
                text(&d.basis),
                text(&d.price_basis)
            );
            match row.claims() {
                Ok(claims) => {
                    let report = &claims.derived.value;
                    let _ = writeln!(
                        out,
                        "{}: published {}/{} {}; computed {}/{}; exact input-rounding enclosure [{}/{}, {}/{}].\n",
                        report.state.as_str(),
                        report.published.value.num(),
                        report.published.value.den(),
                        report.unit.symbol(),
                        report.computed.num(),
                        report.computed.den(),
                        report.low.num(),
                        report.low.den(),
                        report.high.num(),
                        report.high.den()
                    );
                    let _ = writeln!(
                        out,
                        "Derived Claim `{}`; issuer assertion; deterministic arithmetic.\n",
                        claims.derived.claim_id
                    );
                    for input in &claims.inputs {
                        let _ = writeln!(
                            out,
                            "- Input Claim `{}`: {} / {} (issuer assertion).",
                            input.claim_id,
                            input.value.role.as_str(),
                            text(&input.value.quantity)
                        );
                    }
                    out.push('\n');
                }
                Err(reason) => {
                    let _ = writeln!(out, "Refused: {}\n", text(&reason));
                }
            }
            for document in &row.documents {
                let _ = writeln!(
                    out,
                    "Source {} ({}, published {}; {}), SHA-256 `{}`.\n",
                    text(&document.file),
                    text(&document.kind),
                    text(&document.published_at),
                    text(&document.published_at_basis),
                    text(&document.sha256)
                );
            }
            for input in &d.inputs {
                let candidates: Vec<_> = row
                    .evidence
                    .iter()
                    .filter(|a| a.quantity == input.quantity)
                    .collect();
                if candidates.is_empty() {
                    let _ = writeln!(
                        out,
                        "- {}: {} — no agreed source reading.",
                        input.role.as_str(),
                        text(&input.quantity)
                    );
                }
                for a in candidates {
                    let _ = writeln!(
                        out,
                        "- {}: {} {} at {}; basis {}; period {}; readers {}; SHA-256 `{}`.",
                        input.role.as_str(),
                        text(&a.token),
                        text(&a.unit),
                        text(&a.locator.to_string()),
                        text(&a.basis),
                        text(&a.period),
                        text(&a.readers.join(", ")),
                        text(&a.doc_sha256)
                    );
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_registry::table_identities::cutoff::CutoffInput;
    use fmine_registry::transcriptions::{Locator, parse_token};

    struct Fixture {
        declarations: Vec<CutoffDeclaration>,
        readings: Vec<Accepted>,
        documents: Vec<DocumentRow>,
    }

    impl Fixture {
        fn report(&self) -> SourceReport {
            assess(&self.declarations, &self.readings, &self.documents)
        }

        fn add(&mut self, role: InputRole, quantity: &str, token: &str) {
            let d = &self.declarations[0];
            self.readings.push(Accepted {
                locator: Locator {
                    doc: d.doc.clone(),
                    page: 2,
                    table: "Invented technical inputs".into(),
                    row: quantity.into(),
                    col: "Assumption".into(),
                },
                doc_sha256: self.documents[0].sha256.clone(),
                token: token.into(),
                value: parse_token(token).unwrap(),
                quantity: quantity.into(),
                unit: role.unit(d.unit).into(),
                basis: if role == InputRole::GoldPrice {
                    &d.price_basis
                } else {
                    &d.basis
                }
                .clone(),
                period: d.period.clone(),
                readers: vec!["reader_a".into(), "reader_b".into()],
            });
            self.declarations[0].inputs.push(CutoffInput {
                role,
                quantity: quantity.into(),
            });
        }

        fn reading(&mut self, role: InputRole) -> &mut Accepted {
            let quantity = &self.declarations[0]
                .inputs
                .iter()
                .find(|i| i.role == role)
                .unwrap()
                .quantity;
            self.readings
                .iter_mut()
                .find(|a| a.quantity == *quantity)
                .unwrap()
        }

        fn set(&mut self, role: InputRole, token: &str) {
            let reading = self.reading(role);
            reading.token = token.into();
            reading.value = parse_token(token).unwrap();
        }
    }

    fn fixture(model: RecoveryModel) -> Fixture {
        let declaration = CutoffDeclaration {
            id: "invented_cutoff".into(),
            doc: "Mine.pdf".into(),
            subject: "Invented Mine".into(),
            period: "effective_2024_12_31".into(),
            basis: "full;resource".into(),
            price_basis: "resource_price".into(),
            unit: Unit::GramsPerTonne,
            recovery: model,
            inputs: Vec::new(),
        };
        let document = DocumentRow {
            file: "Mine.pdf".into(),
            dir: "documents".into(),
            kind: "technical_report".into(),
            url: "https://example.invalid/Mine.pdf".into(),
            url_basis: "invented fixture".into(),
            published_at: "2025-03-01".into(),
            published_at_basis: "FACT: invented date".into(),
            pages: 10,
            bytes: 100,
            sha256: "a".repeat(64),
        };
        let mut fixture = Fixture {
            declarations: vec![declaration],
            readings: Vec::new(),
            documents: vec![document],
        };
        fixture.add(
            InputRole::Published,
            "cutoff_published",
            if model == RecoveryModel::ConstantPercent {
                "2.00"
            } else {
                "1.50"
            },
        );
        // Exactly 100 USD per gram: no decimal approximation to the ounce conversion.
        fixture.add(InputRole::GoldPrice, "cutoff_gold_price", "3110.34768");
        fixture.add(InputRole::Cost, "cutoff_cost", "100.0");
        match model {
            RecoveryModel::ConstantPercent => {
                fixture.add(InputRole::Recovery, "cutoff_recovery", "50.0")
            }
            RecoveryModel::TcmOverGold => {
                fixture.add(InputRole::RecoveryIntercept, "cutoff_intercept", "100.0");
                fixture.add(InputRole::RecoverySlope, "cutoff_slope", "50.0");
                fixture.add(InputRole::Tcm, "cutoff_tcm", "1.00");
            }
        }
        fixture
    }

    #[test]
    fn exact_constant_and_fixed_point_answers_keep_all_source_evidence() {
        for (model, value) in [
            (RecoveryModel::ConstantPercent, ExactQty::from_integer(2)),
            (RecoveryModel::TcmOverGold, ExactQty::new(3, 2).unwrap()),
        ] {
            let f = fixture(model);
            let report = f.report();
            let result = report.rows[0].result.as_ref().unwrap();
            assert_eq!(result.computed, value);
            assert_eq!(result.state, super::super::CutoffState::Reproduced);
            assert_eq!(
                result.solver_iterations.is_some(),
                model == RecoveryModel::TcmOverGold
            );
            assert_eq!(report.rows[0].evidence.len(), f.readings.len());
            let json = report.to_json();
            assert!(json.contains("\"gold_price\""));
            assert!(json.contains("\"price_basis\":\"resource_price\""));
            assert!(json.contains("\"token\":\"3110.34768\""));
            assert!(json.contains("\"exact\":{\"published\":"));
            let markdown = report.to_markdown();
            assert!(
                markdown.contains("Mine.pdf:p2:Invented technical inputs:cutoff_cost/Assumption")
            );
            assert!(markdown.contains(&"a".repeat(64)));
            assert!(markdown.contains("reader_a, reader_b"));
        }
    }

    #[test]
    fn short_ton_output_and_multiple_named_deductions_use_exact_units() {
        let mut f = fixture(RecoveryModel::ConstantPercent);
        f.declarations[0].unit = Unit::OuncesPerShortTon;
        f.reading(InputRole::Published).unit = "oz_per_short_ton".into();
        f.set(InputRole::Published, "0.0583");
        f.add(InputRole::PublishedMetric, "cutoff_metric", "2.00");
        let report = f.report();
        let result = report.rows[0].result.as_ref().unwrap();
        assert_eq!(result.computed, ExactQty::new(7, 120).unwrap());
        assert_eq!(result.metric_from_rounded, Some(true));

        let mut f = fixture(RecoveryModel::ConstantPercent);
        f.add(InputRole::Cost, "cutoff_sustaining", "20.0");
        f.add(InputRole::RevenueDeduction, "cutoff_tax", "10.0");
        f.add(InputRole::OunceDeduction, "cutoff_royalty", "311.034768");
        f.set(InputRole::Published, "3.00");
        assert_eq!(
            f.report().rows[0].result.as_ref().unwrap().computed,
            ExactQty::from_integer(3)
        );
        f.reading(InputRole::Cost).unit = "usd_per_short_ton".into();
        assert!(f.report().rows[0].result.is_err());
    }

    #[test]
    fn source_identity_value_and_agreement_mutations_refuse_without_dropping_evidence() {
        for mutation in 0..12 {
            let mut f = fixture(RecoveryModel::ConstantPercent);
            match mutation {
                0 => f.reading(InputRole::Cost).unit = "usd_per_short_ton".into(),
                1 => f.reading(InputRole::Cost).basis = "attributable:1/2;resource".into(),
                2 => f.reading(InputRole::GoldPrice).basis = "realized_price".into(),
                3 => f.reading(InputRole::Cost).period = "effective_2023_12_31".into(),
                4 => f.reading(InputRole::Cost).doc_sha256 = "b".repeat(64),
                5 => f.reading(InputRole::Cost).readers = vec!["reader_a".into()],
                6 => {
                    f.reading(InputRole::Cost).readers = vec!["reader_a".into(), "reader_a".into()]
                }
                7 => f.set(InputRole::Cost, "99.0 - 101.0"),
                8 => f.reading(InputRole::Cost).value = parse_token("999.0").unwrap(),
                9 => f.documents[0].kind = "investor_deck".into(),
                10 => f.reading(InputRole::Cost).locator.page = 11,
                _ => {
                    let mut duplicate = f.reading(InputRole::Cost).clone();
                    duplicate.locator.row = "Another cost location".into();
                    f.readings.push(duplicate);
                }
            }
            let report = f.report();
            assert!(report.rows[0].result.is_err(), "mutation {mutation}");
            assert_eq!(report.rows[0].evidence.len(), f.readings.len());
            assert!(
                report
                    .to_json()
                    .contains("\"status\":\"refused\",\"result\":null")
            );
        }
    }

    #[test]
    fn unphysical_domains_refuse_but_a_valid_intercept_above_one_hundred_is_admitted() {
        for (role, token) in [
            (InputRole::Published, "-2.00"),
            (InputRole::Cost, "-100.0"),
            (InputRole::Cost, "0.0"),
            (InputRole::GoldPrice, "0"),
            (InputRole::Recovery, "0.0"),
            (InputRole::Recovery, "101.0"),
        ] {
            let mut f = fixture(RecoveryModel::ConstantPercent);
            f.set(role, token);
            let report = f.report();
            assert!(report.rows[0].result.is_err(), "{role:?} {token}");
            assert!(
                report
                    .to_json()
                    .contains(&format!("\"token\":{}", js(token)))
            );
        }
        let mut f = fixture(RecoveryModel::TcmOverGold);
        f.set(InputRole::Tcm, "101.0");
        assert!(
            f.report().rows[0]
                .result
                .as_ref()
                .unwrap_err()
                .contains("TCM in [0,100]")
        );
        f.set(InputRole::Tcm, "1.00");
        f.set(InputRole::RecoveryIntercept, "150.0");
        f.set(InputRole::RecoverySlope, "1.00");
        f.set(InputRole::Tcm, "0.010");
        assert!(
            f.report().rows[0]
                .result
                .as_ref()
                .unwrap_err()
                .contains("effective recovery")
        );

        // Existing E182 numerical oracle, under invented fixture citations; this is a binding test,
        // not independent transcription or a claim that the real report bytes were available here.
        f.set(InputRole::Published, "5.66");
        f.set(InputRole::GoldPrice, "1700");
        f.set(InputRole::Cost, "243.89");
        f.set(InputRole::RecoveryIntercept, "100.79");
        f.set(InputRole::RecoverySlope, "155.7");
        f.set(InputRole::Tcm, "0.771");
        f.add(InputRole::RevenueDeduction, "cutoff_tax", "1.02");
        f.add(InputRole::RevenueDeduction, "cutoff_royalty", "0.00");
        f.add(InputRole::OunceDeduction, "cutoff_refining", "0.38");
        assert_eq!(
            f.report().rows[0].result.as_ref().unwrap().state,
            super::super::CutoffState::Reproduced
        );
    }

    #[test]
    fn partial_refusals_are_retained_and_all_relevant_inputs_remain_digest_bound() {
        let mut f = fixture(RecoveryModel::ConstantPercent);
        let mut other = f.declarations[0].clone();
        other.id = "other_cutoff".into();
        other.doc = "Other.pdf".into();
        f.declarations.push(other);
        let mut doc = f.documents[0].clone();
        doc.file = "Other.pdf".into();
        f.documents.push(doc);
        let mut readings = f.readings.clone();
        for reading in &mut readings {
            reading.locator.doc = "Other.pdf".into();
        }
        f.readings.extend(readings);
        f.readings
            .retain(|a| !(a.locator.doc == "Mine.pdf" && a.quantity == "cutoff_cost"));
        let report = f.report();
        assert_eq!(report.rows.len(), 2);
        assert!(report.rows[0].result.is_err());
        assert!(report.rows[1].result.is_ok());
        f.declarations.reverse();
        for d in &mut f.declarations {
            d.inputs.reverse();
        }
        f.documents.reverse();
        f.readings.reverse();
        for reading in &mut f.readings {
            reading.readers.reverse();
        }
        assert_eq!(report.to_json(), f.report().to_json());
        f.documents
            .iter_mut()
            .find(|d| d.file == "Mine.pdf")
            .unwrap()
            .url_basis = "changed rejected source metadata".into();
        assert_ne!(report.digest(), f.report().digest());
        let before = f.report().digest();
        f.readings
            .iter_mut()
            .find(|a| a.locator.doc == "Mine.pdf")
            .unwrap()
            .value = parse_token("123.0").unwrap();
        assert_ne!(before, f.report().digest());
    }

    #[test]
    fn source_change_moves_computation_and_declared_conflicts_are_not_corrected() {
        let mut f = fixture(RecoveryModel::ConstantPercent);
        let before = f.report();
        f.set(InputRole::Cost, "200.0");
        let changed = f.report();
        assert_ne!(before.digest(), changed.digest());
        let result = changed.rows[0].result.as_ref().unwrap();
        assert_eq!(result.computed, ExactQty::from_integer(4));
        assert_eq!(result.published.value, ExactQty::from_integer(2));
        assert_eq!(result.state, super::super::CutoffState::Conflict);
        assert!(changed.to_json().contains("\"token\":\"2.00\""));
        assert_eq!(
            changed.rows[0].evidence.len(),
            before.rows[0].evidence.len()
        );
    }

    #[test]
    fn canonical_claims_preserve_issuer_evidence_and_complete_ancestry() {
        use fmine_claim::{AdditivityClass, Representation};
        use fmine_evidence::EvidenceClass;
        let mut f = fixture(RecoveryModel::TcmOverGold);
        let report = f.report();
        let row = &report.rows[0];
        let claims = row.claims().unwrap();
        assert_eq!(claims.inputs.len(), f.readings.len());
        let mut parents = Vec::new();
        let mut anchors = Vec::new();
        for input in &claims.inputs {
            input.check_id().unwrap();
            assert_eq!(input.evidence, EvidenceClass::ISSUER);
            assert_eq!(input.representation, Representation::Deterministic);
            assert_eq!(input.additivity, AdditivityClass::NonAdditive);
            assert!(input.parents.is_empty());
            assert!(input.color().is_err());
            parents.push(input.claim_id);
            anchors.extend(input.anchors.clone());
        }
        parents.sort();
        anchors.sort();
        anchors.dedup();
        assert_eq!(claims.derived.parents, parents);
        assert_eq!(claims.derived.anchors, anchors);
        assert_eq!(claims.derived.evidence, EvidenceClass::ISSUER);
        assert_eq!(claims.derived.value, *row.result.as_ref().unwrap());
        assert!(claims.derived.color().is_err());
        claims.derived.check_id().unwrap();
        row.check_claims(&claims).unwrap();
        assert!(
            report
                .to_json()
                .contains(&claims.derived.claim_id.to_string())
        );
        assert!(
            report
                .to_markdown()
                .contains(&claims.derived.claim_id.to_string())
        );
        f.declarations[0].inputs.reverse();
        f.readings.reverse();
        for reading in &mut f.readings {
            reading.readers.reverse();
        }
        assert_eq!(claims, f.report().rows[0].claims().unwrap());
    }

    #[test]
    fn claim_identity_binds_source_precision_metadata_and_declared_semantics() {
        let f = fixture(RecoveryModel::ConstantPercent);
        let before = f.report().rows[0].claims().unwrap();
        for mutation in 0..5 {
            let mut f = fixture(RecoveryModel::ConstantPercent);
            match mutation {
                // Gold price is exact in this model; changing only its original printed precision
                // leaves every arithmetic answer unchanged but must change the source ancestry.
                0 => f.set(InputRole::GoldPrice, "3110.347680"),
                1 => f.documents[0].url_basis = "different provenance statement".into(),
                2 => f.documents[0].bytes += 1,
                3 => f.reading(InputRole::Cost).readers[0] = "reader_c".into(),
                _ => f.declarations[0].subject = "Another declared subject".into(),
            }
            let changed = f.report().rows[0].claims().unwrap();
            assert_eq!(before.derived.value, changed.derived.value);
            assert_ne!(
                before.derived.claim_id, changed.derived.claim_id,
                "{mutation}"
            );
            assert_ne!(
                before.derived.parents, changed.derived.parents,
                "{mutation}"
            );
        }
    }

    #[test]
    fn typed_claim_and_result_mutations_refuse_instead_of_minting_false_provenance() {
        use fmine_claim::Representation;
        let mut report = fixture(RecoveryModel::ConstantPercent).report();
        let row = &report.rows[0];
        let original = row.claims().unwrap();
        for mutation in 0..5 {
            let mut claims = original.clone();
            match mutation {
                0 => claims.inputs[0].value.printed.value = ExactQty::from_integer(999),
                1 => claims.derived.value.computed = ExactQty::from_integer(999),
                2 => claims.derived.representation = Representation::Interval { lo: 0.0, hi: 1.0 },
                3 => claims.derived.parents.clear(),
                _ => claims.derived.stale = true,
            }
            assert!(row.check_claims(&claims).is_err(), "{mutation}");
        }
        report.rows[0].result.as_mut().unwrap().computed = ExactQty::from_integer(999);
        assert!(report.rows[0].claims().is_err());
        assert!(
            report
                .to_json()
                .contains("\"status\":\"refused\",\"result\":null")
        );
        assert!(report.to_json().contains("\"claims\":null"));
        assert!(
            report
                .to_markdown()
                .contains("stored cutoff result differs")
        );
        assert_eq!(report.rows[0].evidence.len(), original.inputs.len());
    }
}
