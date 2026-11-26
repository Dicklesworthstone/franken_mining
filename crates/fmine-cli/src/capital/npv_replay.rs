//! Read-only developer valuation replay over a cited local worksheet (BD-350.S4).
//! Determinism: Exact

use std::fmt::Write as _;
use std::path::Path;

use fmine_econ::EconError;
use fmine_econ::npv::{
    ConventionDiscoveryRequest, DiscountConvention, ExactInterval, TaxBasis,
    discover_discounting_convention, solve_irr_bracketed,
};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::parse_subset;
use fmine_units::ExactQty;

use super::investment::{Fields, fields, get, invalid, number};
use crate::{Envelope, json_string as js, worksheet_input};

pub(crate) const VERB: &str = "check developer-npv";
pub(crate) const USAGE: &str = "check developer-npv <worksheet.toml>";
const SCHEMA: &str = "fmine.developer-npv.v1";
const NO_CLAIM: &str = "Replay of reader-transcribed annual cash flows in USD millions. A compatible convention overlaps the stated printed-NPV rounding interval within the three tested timing rules. The worksheet is the retained source. The reader supplies original document locations, cash-flow scope and tax treatment. A numerical IRR is conditional on the exact supplied schedule and explicit bracket. Issuer authentication and investment authorization require separate evidence.";
const MAX_BYTES: u64 = 64 * 1024;
const MAX_FLOWS: usize = 64;
const AT_VALUATION: &str = "valuation_date";
const END_FIRST_PERIOD: &str = "end_of_first_period";

type Result<T> = std::result::Result<T, Refusal>;

fn exact(qty: ExactQty) -> String {
    format!("{}/{}", qty.num(), qty.den())
}

fn interval(bounds: ExactInterval) -> String {
    format!("[\"{}\",\"{}\"]", exact(bounds.lo), exact(bounds.hi))
}

fn original_source(top: &Fields<'_>, evidence: &str) -> Result<String> {
    match (top.get("source_url"), top.get("report_sha256")) {
        (None, None) if evidence == "synthetic" => Ok("null".into()),
        (Some(url), Some(hash))
            if evidence == "declared_public"
                && url.starts_with("https://")
                && fmine_calib::record::parse_hex32(hash).is_some() =>
        {
            Ok(format!(
                "{{\"url\":{},\"declared_sha256\":{}}}",
                js(url),
                js(hash)
            ))
        }
        _ => Err(invalid(
            "declared_public requires an HTTPS source_url and a lowercase report_sha256",
        )),
    }
}

fn incomplete_tax_schedule() -> Refusal {
    Refusal::new(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        "explicit tax_outflow in every year is required for after-tax replay",
    )
}

fn irr(
    top: &Fields<'_>,
    flow_step: ExactQty,
    flows: &[ExactQty],
    basis: &TaxBasis,
    convention: DiscountConvention,
) -> Result<(String, String)> {
    let (lo, hi) = match (top.get("irr_lo"), top.get("irr_hi")) {
        (Some(lo), Some(hi)) => (number(lo)?, number(hi)?),
        (None, None) => return Ok((r#"{"status":"not_requested"}"#.into(), String::new())),
        _ => return Err(invalid("irr_lo and irr_hi must be supplied together")),
    };
    if flow_step.signum() != 0 {
        return Err(invalid(
            "an IRR bracket requires flow_step = 0 for the supplied flows",
        ));
    }
    let tol = number("1/1000000000")?;
    let bracket = format!("[\"{}\",\"{}\"]", exact(lo), exact(hi));
    match solve_irr_bracketed(flows, convention, basis, lo, hi, tol, 128) {
        Ok(root) => Ok((
            format!(
                "{{\"status\":\"converged\",\"rate\":\"{}\",\"bracket\":{bracket},\"iterations\":{}}}",
                exact(root.rate),
                root.iterations,
            ),
            format!("IRR ≈{:.4}%.\n\n", root.rate.to_f64() * 100.0),
        )),
        Err(error) => {
            let refusal = error.to_refusal();
            Ok((
                format!(
                    "{{\"status\":\"refused\",\"bracket\":{bracket},\"code\":{},\"what\":{}}}",
                    js(refusal.code),
                    js(&refusal.what),
                ),
                format!(
                    "IRR refused `{}`. {}\n\n",
                    refusal.code,
                    worksheet_input::markdown_cell(&refusal.what)
                ),
            ))
        }
    }
}

fn evaluate(path: &Path) -> Result<Envelope> {
    let (text, source) = worksheet_input::read(path, "developer NPV worksheet", MAX_BYTES)?;
    let doc = parse_subset(&text).map_err(|e| invalid(e.to_string()))?;
    let top = fields(
        &doc.top,
        &[
            "schema",
            "document",
            "evidence",
            "tax_basis",
            "discount_rate",
            "rate_locator",
            "printed_npv",
            "npv_locator",
            "printed_step",
            "flow_step",
        ],
        &[
            "irr_lo",
            "irr_hi",
            "year_zero_timing",
            "source_url",
            "report_sha256",
        ],
    )?;
    let document = get(&top, "document")?;
    let evidence = get(&top, "evidence")?;
    if get(&top, "schema")? != SCHEMA || !matches!(evidence, "synthetic" | "declared_public") {
        return Err(invalid(format!(
            "{SCHEMA} requires evidence synthetic or declared_public"
        )));
    }
    if !(2..=MAX_FLOWS).contains(&doc.rows.len()) {
        return Err(invalid("provide 2–64 annual flow rows including year zero"));
    }
    let source_document = original_source(&top, evidence)?;
    let basis = get(&top, "tax_basis")?;
    let after_tax = match basis {
        "pre_tax" => false,
        "after_tax" => true,
        _ => return Err(invalid("tax_basis requires pre_tax or after_tax")),
    };
    let discount_rate = number(get(&top, "discount_rate")?)?;
    let printed_npv = number(get(&top, "printed_npv")?)?;
    let printed_step = number(get(&top, "printed_step")?)?;
    let flow_step = number(get(&top, "flow_step")?)?;
    let timing = top.get("year_zero_timing").copied().unwrap_or(AT_VALUATION);
    let arithmetic = |error| EconError::from(error).to_refusal();
    let one = ExactQty::from_integer(1);
    let scale = match timing {
        AT_VALUATION => one,
        END_FIRST_PERIOD => one.checked_add(discount_rate).map_err(arithmetic)?,
        _ => {
            return Err(invalid(
                "year_zero_timing requires valuation_date or end_of_first_period",
            ));
        }
    };
    if scale.signum() <= 0 {
        return Err(invalid("discount_rate must satisfy 1 + rate > 0"));
    }
    let mut flows = Vec::with_capacity(doc.rows.len());
    let mut taxes = Vec::new();
    let mut citations = Vec::with_capacity(doc.rows.len());
    for (year, row) in doc.rows.iter().enumerate() {
        if row.table != "flow" {
            return Err(invalid(format!("line {}: expected a flow row", row.line)));
        }
        let f = fields(
            &row.fields,
            &["year", "amount", "locator"],
            &["tax_outflow", "tax_locator"],
        )?;
        if get(&f, "year")?.parse::<usize>().ok() != Some(year) {
            return Err(invalid(format!(
                "line {}: expected year {year} in consecutive order",
                row.line
            )));
        }
        let amount = number(get(&f, "amount")?)?;
        flows.push(amount);
        let tax = match (after_tax, f.get("tax_outflow"), f.get("tax_locator")) {
            (false, None, None) => String::new(),
            (true, Some(value), Some(locator)) => {
                let tax = number(value)?;
                taxes.push(tax);
                format!(
                    ",\"tax_outflow_usd_m\":\"{}\",\"tax_locator\":{}",
                    exact(tax),
                    js(locator)
                )
            }
            (true, _, _) => return Err(incomplete_tax_schedule()),
            _ => {
                return Err(invalid(
                    "pre_tax flow rows must omit tax_outflow and tax_locator",
                ));
            }
        };
        citations.push(format!(
            "{{\"year\":{year},\"worksheet_line\":{},\"locator\":{},\"cash_flow_usd_m\":\"{}\"{tax}}}",
            row.line, js(get(&f, "locator")?), exact(amount),
        ));
    }
    let tax_basis = if after_tax {
        TaxBasis::AfterTax {
            tax_outflows: Some(taxes),
        }
    } else {
        TaxBasis::PreTax
    };
    let digest = fmine_core::Digest32(source.artifact_sha256);
    // Compare at the kernel's year-zero origin. A valuation one year earlier discounts every
    // NPV by the same positive factor. Scale the target and its step together before matching.
    let result = discover_discounting_convention(&ConventionDiscoveryRequest {
        report_id: document,
        report_sha256: &digest.to_hex(),
        flows: &flows,
        flow_rounding_step: flow_step,
        discount_rate,
        printed_npv: printed_npv.checked_mul(scale).map_err(arithmetic)?,
        printed_step: printed_step.checked_mul(scale).map_err(arithmetic)?,
        tax_basis: &tax_basis,
    })
    .map_err(|error| {
        let mut refusal = error.to_refusal();
        if timing == END_FIRST_PERIOD && matches!(error, EconError::NonIdentifiableConvention { .. })
        {
            refusal.what.push_str(
                ". Bounds in this diagnostic use the year-zero reference, with the printed NPV scaled by 1 + discount_rate.",
            );
        }
        refusal
    })?;
    let convention = result.discovered.as_str();
    let at_valuation = |bounds: ExactInterval| -> Result<ExactInterval> {
        Ok(ExactInterval {
            lo: bounds.lo.checked_div(scale).map_err(arithmetic)?,
            hi: bounds.hi.checked_div(scale).map_err(arithmetic)?,
        })
    };
    let (irr_json, irr_md) = irr(&top, flow_step, &flows, &tax_basis, result.discovered)?;
    let npv_locator = get(&top, "npv_locator")?;
    let rate_locator = get(&top, "rate_locator")?;
    let cell = worksheet_input::markdown_cell;
    let mut md = format!(
        "# Developer NPV replay\n\n{} · {} · USD millions\n\nWorksheet SHA-256 `{digest}`\n\nCompatible convention **{convention}** with year zero at `{timing}`.\n\nPrinted NPV ≈{:.4} from {}. Discount rate ≈{:.4}% from {}.\n\n| Convention | NPV low (approx.) | NPV high (approx.) | Overlaps printed NPV |\n|---|---:|---:|---|\n",
        cell(document),
        cell(basis),
        printed_npv.to_f64(),
        cell(npv_locator),
        discount_rate.to_f64() * 100.0,
        cell(rate_locator),
    );
    let mut comparisons = Vec::with_capacity(result.evaluations.len());
    for row in result.evaluations {
        let name = row.convention.as_str();
        let bounds = at_valuation(row.npv_interval)?;
        comparisons.push(format!(
            "{{\"convention\":\"{name}\",\"npv_bounds_usd_m\":{},\"overlaps_printed\":{}}}",
            interval(bounds),
            row.matches_printed,
        ));
        let _ = writeln!(
            md,
            "| {name} | {:.4} | {:.4} | {} |",
            bounds.lo.to_f64(),
            bounds.hi.to_f64(),
            row.matches_printed
        );
    }
    let _ = write!(md, "\n{irr_md}Use JSON for exact values. {NO_CLAIM}\n");
    let mut envelope = Envelope::ok(
        VERB,
        format!(
            "{{\"schema\":\"{SCHEMA}\",\"worksheet\":{},\"source_document\":{source_document},\"document\":{},\"evidence\":{},\"tax_basis\":{},\"year_zero_timing\":\"{timing}\",\"discount_rate\":\"{}\",\"rate_locator\":{},\"printed_npv_usd_m\":\"{}\",\"npv_locator\":{},\"printed_bounds_usd_m\":{},\"flow_step_usd_m\":\"{}\",\"compatible_convention\":\"{convention}\",\"comparisons\":[{}],\"flows\":[{}],\"irr\":{irr_json}}}",
            worksheet_input::source_json(&source, text.len()),
            js(document),
            js(evidence),
            js(basis),
            exact(discount_rate),
            js(rate_locator),
            exact(printed_npv),
            js(npv_locator),
            interval(at_valuation(result.printed_interval)?),
            exact(flow_step),
            comparisons.join(","),
            citations.join(","),
        ),
        Vec::new(),
    );
    envelope.no_claim = Some(NO_CLAIM);
    envelope.rendered_markdown = Some(md);
    Ok(envelope)
}

pub(crate) fn execute(path: &Path, root: &Path) -> Envelope {
    evaluate(&root.join(path)).unwrap_or_else(|r| Envelope::refused(VERB, r, Vec::new()))
}
