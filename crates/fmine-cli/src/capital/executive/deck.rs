//! DESIGN (bd-xm5k): replay dated committee prices over admitted source investments.
//! Determinism: DeterministicPerIsa (source production receipts).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use fmine_claim::Claim;
use fmine_core::codec::Encoder;
use fmine_core::hash::sha256;
use fmine_evidence::RefusalCode;
use fmine_metrics::valuation::{self, investment as model};
use fmine_units::{ExactQty as Rat, UnitsError};

use super::super::investment::{compatible, number};
use super::*;
use crate::investment::{LoadedInvestment, load};

const ZERO: Rat = Rat::from_integer(0);
const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

type Prices<'a> = BTreeMap<(&'a str, &'a str), (Rat, usize)>;

fn exact<T>(result: std::result::Result<T, UnitsError>) -> Result<T> {
    result.map_err(|e| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("common deck exact arithmetic: {e}"),
        )
    })
}

fn ratio(value: Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn usd(value: Rat) -> String {
    // Weighted amounts can have more precision than Money. Retain the exact
    // rational representation when a four-place decimal would lose precision.
    if let Ok(minor) = value.checked_mul(Rat::from_integer(Money::SCALE))
        && minor.den() == 1
    {
        let n = minor.num().unsigned_abs();
        let sign = if minor.signum() < 0 { "-" } else { "" };
        let scale = Money::SCALE as u128;
        let mut decimals = format!("{:04}", n % scale);
        while decimals.len() > 2 && decimals.ends_with('0') {
            decimals.pop();
        }
        return format!("{sign}{}.{decimals}", dollars((n / scale) as i128));
    }
    format!("US$ {}", ratio(value))
}

fn replay(
    original: &model::InvestmentReceipt,
    prices: &Prices<'_>,
    source: &SourceAnchor,
) -> Result<model::InvestmentReceipt> {
    let mut bundles = Vec::with_capacity(original.bundles().len());
    for bundle in original.bundles() {
        let old = bundle.valuation.terms();
        let mut terms = old.value.clone();
        let mut anchors = old.anchors.clone();
        for scenario in &mut terms.scenarios {
            let &(price, line) = prices
                .get(&(bundle.period_id.as_str(), scenario.id.as_str()))
                .ok_or_else(|| {
                    invalid("missing committee price for an investment state and period")
                })?;
            scenario.usd_per_kg = price;
            anchors.push(SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {line}: price.usd_per_kg"),
            ));
        }
        let tag = terms.canonical_tag();
        let repriced = Claim::new(
            valuation::INPUT_METRIC,
            terms,
            &tag,
            old.representation.clone(),
            old.support,
            old.additivity,
            old.evidence,
            old.epistemic,
            old.state,
            old.confidentiality().clone(),
            old.uu_terms.clone(),
            vec![old.claim_id],
            anchors,
        );
        bundles.push(model::PeriodValuation {
            alternative_id: bundle.alternative_id.clone(),
            period_id: bundle.period_id.clone(),
            valuation: valuation::evaluate(bundle.valuation.production(), &repriced)?,
        });
    }
    model::evaluate(original.declaration(), &bundles)
}

fn mean(receipt: &model::InvestmentReceipt, id: &str) -> Result<Rat> {
    receipt
        .decision()
        .summary()
        .alternatives
        .iter()
        .find(|row| row.action_id == id)
        .map(|row| row.mean.major)
        .ok_or_else(|| invalid(format!("unknown source investment alternative {id}")))
}

fn winners(values: [Rat; 3], names: [&str; 3]) -> Result<Vec<String>> {
    let mut best = ZERO;
    for value in values {
        if exact(value.checked_cmp(best))? == Ordering::Greater {
            best = value;
        }
    }
    Ok(values
        .iter()
        .zip(names)
        .filter(|(value, _)| **value == best)
        .map(|(_, name)| name.to_owned())
        .collect())
}

struct Candidate<'a> {
    label: &'a str,
    path: &'a str,
    alternative: &'a str,
}

pub(super) fn compare(
    doc: &TomlDoc,
    as_of: &str,
    source: &SourceAnchor,
    root: &Path,
    full_sources: bool,
) -> Result<Card> {
    tables(doc, &["comparison", "candidate", "price"])?;
    let basis = row_fields(
        table(doc, "comparison")?,
        &["element", "ownership_basis", "repricing_basis"],
        as_of,
    )?;
    let element = get(&basis, "element")?;
    let ownership = get(&basis, "ownership_basis")?;
    if get(&basis, "repricing_basis")? != "fixed_plan_flat_royalty_pre_tax" {
        return Err(invalid(
            "common deck requires fixed_plan_flat_royalty_pre_tax; price-dependent mine plans or fiscal terms need a full revaluation",
        ));
    }

    let candidates = doc
        .rows_of("candidate")
        .map(|row| {
            let f = row_fields(row, &["label", "investment", "alternative"], as_of)?;
            let alternative = get(&f, "alternative")?;
            if alternative == valuation::DO_NOTHING {
                return Err(invalid("hold cash is added automatically"));
            }
            Ok(Candidate {
                label: get(&f, "label")?,
                path: get(&f, "investment")?,
                alternative,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let [first, second] = candidates.as_slice() else {
        return Err(invalid("exactly two candidate rows are required"));
    };
    if first.label == second.label
        || candidates
            .iter()
            .any(|c| c.label.eq_ignore_ascii_case("Hold cash"))
        || (first.path == second.path && first.alternative == second.alternative)
    {
        return Err(invalid(
            "two distinct investment choices and labels are required",
        ));
    }
    let mut prices = Prices::new();
    for row in doc.rows_of("price") {
        let f = row_fields(row, &["period", "state", "usd_per_kg"], as_of)?;
        let price = number(get(&f, "usd_per_kg")?)?;
        let key = (get(&f, "period")?, get(&f, "state")?);
        if price.signum() < 0 || prices.insert(key, (price, row.line)).is_some() {
            return Err(invalid(
                "committee prices must be nonnegative and unique per period and state",
            ));
        }
    }

    let mut original = BTreeMap::<&str, LoadedInvestment>::new();
    for c in &candidates {
        if let std::collections::btree_map::Entry::Vacant(entry) = original.entry(c.path) {
            entry.insert(load(Path::new(c.path), root)?);
        }
    }
    let reference = original[first.path].receipt();
    let spec = &reference.declaration().value;
    let states = &reference.decision().inputs().weights.value.states;
    if prices.len() != spec.periods.len() * states.len()
        || spec.periods.iter().any(|p| {
            states
                .iter()
                .any(|(id, _)| !prices.contains_key(&(p.id.as_str(), id.as_str())))
        })
    {
        return Err(invalid(
            "supply exactly one committee price for every source period and state",
        ));
    }
    if original.values().any(|investment| {
        let receipt = investment.receipt();
        let current = &receipt.declaration().value;
        current.element != element
            || !compatible(spec, current)
            || spec.shared_scenario_basis != current.shared_scenario_basis
            || *states != receipt.decision().inputs().weights.value.states
            || current.downside_limit.is_some()
            || !current.information_options.is_empty()
    }) {
        return Err(invalid(
            "common deck requires the declared element and matching periods, valuation time, scenario definitions, weights, and cash and discount bases; use source investments without separate risk screens or contingent actions",
        ));
    }
    let second_receipt = original[second.path].receipt();
    let second_spec = &second_receipt.declaration().value;
    // Resolve the selected actions before replaying any source.
    let submitted_means = [
        mean(reference, first.alternative)?,
        mean(second_receipt, second.alternative)?,
        ZERO,
    ];
    if first.alternative == second.alternative && spec == second_spec {
        return Err(invalid(
            "two labels refer to the same source investment choice",
        ));
    }
    if first.path != second.path
        && spec.boundary == second_spec.boundary
        && spec.circuit == second_spec.circuit
    {
        let baseline = |receipt: &model::InvestmentReceipt, alternative: &str| {
            receipt
                .cashflows()
                .iter()
                .filter(|flow| flow.alternative_id == alternative)
                .map(|flow| {
                    (
                        (flow.period_id.clone(), flow.scenario_id.clone()),
                        flow.do_nothing_cash_usd,
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        if baseline(reference, first.alternative) != baseline(second_receipt, second.alternative) {
            return Err(invalid(
                "investments on the same circuit require identical do-nothing cash by period and state",
            ));
        }
    }

    let mut replayed = BTreeMap::new();
    let mut sources = Vec::new();
    let mut snapshot_bytes = 0;
    let mut digest = Encoder::new_frame(*b"CDEC", 1);
    digest.put_fixed(&source.artifact_sha256);
    for (&path, investment) in &original {
        let common = replay(investment.receipt(), &prices, source)?;
        let submitted = investment.to_json();
        let restated = common.to_json();
        snapshot_bytes += submitted.len() + restated.len();
        if snapshot_bytes > MAX_SNAPSHOT_BYTES {
            return Err(invalid(
                "original and repriced investment snapshots exceed 64 MiB",
            ));
        }
        digest.put_str(path);
        let submitted_sha256 = sha256(submitted.as_bytes());
        let common_sha256 = sha256(restated.as_bytes());
        digest.put_fixed(&submitted_sha256.0);
        digest.put_fixed(&common_sha256.0);
        sources.push(if full_sources {
            format!(
                r#"{{"investment":{},"submitted":{submitted},"common_deck":{restated}}}"#,
                js(path),
            )
        } else {
            format!(
                r#"{{"investment":{},"submitted_sha256":"{submitted_sha256}","common_deck_sha256":"{common_sha256}"}}"#,
                js(path),
            )
        });
        replayed.insert(path, common);
    }

    let names = [first.label, second.label, "Hold cash"];
    let mut restated_means = [ZERO; 3];
    let mut rows = Vec::new();
    let mut details = Vec::new();
    let mut price_inputs_changed = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let original = original[c.path].receipt();
        let submitted = submitted_means[i];
        let common = mean(&replayed[c.path], c.alternative)?;
        let change = exact(common.checked_sub(submitted))?;
        restated_means[i] = common;
        rows.push(vec![
            c.label.into(),
            usd(submitted),
            usd(common),
            usd(change),
        ]);
        let mut price_changes = Vec::new();
        let mut changed = 0;
        for bundle in original
            .bundles()
            .iter()
            .filter(|b| b.alternative_id == c.alternative)
        {
            for state in &bundle.valuation.terms().value.scenarios {
                let (committee, line) = prices[&(bundle.period_id.as_str(), state.id.as_str())];
                changed += usize::from(state.usd_per_kg != committee);
                price_changes.push(format!(
                    r#"{{"period":{},"state":{},"submitted_usd_per_kg":{},"common_usd_per_kg":{},"price_source_line":{line}}}"#,
                    js(&bundle.period_id), js(&state.id), js(&ratio(state.usd_per_kg)), js(&ratio(committee)),
                ));
            }
        }
        if changed > 0 {
            price_inputs_changed.push(format!("{} ({changed} of {})", c.label, prices.len()));
        }
        details.push(format!(
            r#"{{"label":{},"investment":{},"alternative":{},"submitted_mean_usd":{},"common_mean_usd":{},"change_usd":{},"prices":[{}]}}"#,
            js(c.label), js(c.path), js(c.alternative), js(&ratio(submitted)),
            js(&ratio(common)), js(&ratio(change)), price_changes.join(","),
        ));
    }
    rows.push(vec!["Hold cash".into(), usd(ZERO), usd(ZERO), usd(ZERO)]);
    let before = winners(submitted_means, names)?;
    let after = winners(restated_means, names)?;
    let gap_before = exact(submitted_means[0].checked_sub(submitted_means[1]))?;
    let gap_after = exact(restated_means[0].checked_sub(restated_means[1]))?;
    let reversal = gap_before.signum() * gap_after.signum() == -1;
    let digest = sha256(&digest.finish()).to_hex();

    let mut card = Card::new(&[
        "Alternative",
        "Submitted PV",
        "Common-deck PV",
        "Price effect",
    ]);
    card.findings.push(format!(
        "Submitted prices favour {}. Using the common deck favours {}.",
        before.join(" / "),
        after.join(" / "),
    ));
    if reversal {
        card.findings.push(format!(
            "The two projects reverse order after repricing. The value of {} minus {} moves from {} to {}.",
            first.label, second.label, usd(gap_before), usd(gap_after),
        ));
    }
    if !price_inputs_changed.is_empty() {
        card.findings.push(format!(
            "Period and state prices changed for {}.",
            price_inputs_changed.join(" and "),
        ));
    }
    card.findings.push(format!(
        "Weighted incremental pre-tax USD present value for the declared periods. {} prices in USD/kg. Ownership basis: {ownership}. The operating plans, payability, flat royalty rates and costs are held fixed.",
        element,
    ));
    card.rows = rows;
    card.journal_actions.extend([
        ("first-candidate", "Select the first listed investment"),
        ("second-candidate", "Select the second listed investment"),
        ("hold-cash", "Hold cash"),
    ]);
    let sources_key = if full_sources {
        "source_calculations"
    } else {
        "source_receipts"
    };
    let labels = |names: &[String]| names.iter().map(|s| js(s)).collect::<Vec<_>>().join(",");
    card.detail = Some((
        "price_deck",
        format!(
            r#"{{"element":{},"ownership_basis":{},"repricing_basis":"fixed_plan_flat_royalty_pre_tax","submitted_winners":[{}],"common_winners":[{}],"project_order_reversed":{reversal},"submitted_first_minus_second_usd":{},"common_first_minus_second_usd":{},"candidates":[{}],"{sources_key}":[{}],"digest":{}}}"#,
            js(element),
            js(ownership),
            labels(&before),
            labels(&after),
            js(&ratio(gap_before)),
            js(&ratio(gap_after)),
            details.join(","),
            sources.join(","),
            js(&digest),
        ),
    ));
    card.calculation_digest = Some(digest);
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_io::omf::json::{Json, parse};
    use fmine_registry::toml_subset::parse_subset;

    const DECK: &str = include_str!("../../../../../tests/capital/executive_deck.toml");

    fn execute(input: &str) -> Result<Json> {
        let doc = parse_subset(input).expect("valid TOML");
        let source = SourceAnchor::new(
            "synthetic-deck.toml",
            sha256(input.as_bytes()).0,
            "worksheet",
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (_, detail) = compare(&doc, "2030-04-15", &source, &root, true)?
            .detail
            .expect("deck");
        Ok(parse(detail.as_bytes()).expect("valid result JSON"))
    }

    #[test]
    fn both_exact_co_winners_survive_and_hold_cash_wins_at_zero_prices() {
        let tied = DECK.replacen("usd_per_kg = \"50000\"", "usd_per_kg = \"31322500/513\"", 1);
        let result = execute(&tied).expect("rational committee price");
        let str_json = |s: &str| Json::String(s.to_owned());
        assert_eq!(
            result.get("common_winners"),
            Some(&Json::Array(vec![
                str_json("Base mill upgrade"),
                str_json("Higher-power upgrade"),
            ]))
        );
        assert_eq!(
            result.get("common_first_minus_second_usd"),
            Some(&str_json("0"))
        );
        assert_eq!(
            result.get("project_order_reversed"),
            Some(&Json::Bool(false))
        );
        let zero = DECK.replace("50000", "0").replace("70000", "0");
        let result = execute(&zero).expect("zero price");
        assert_eq!(
            result.get("common_winners"),
            Some(&Json::Array(vec![str_json("Hold cash")]))
        );
    }

    #[test]
    fn separate_compatible_manifests_match_the_combined_case() {
        let separate = DECK.replacen(
            "sponsor_power_investment.toml",
            "production_investment.toml",
            1,
        );
        let result = execute(&separate).expect("two source manifests");
        let combined = execute(DECK).expect("one source manifest");
        for key in [
            "submitted_winners",
            "common_winners",
            "common_first_minus_second_usd",
        ] {
            assert_eq!(result.get(key), combined.get(key));
        }
        assert_ne!(result.get("digest"), combined.get("digest"));
        assert_eq!(
            result
                .get("source_calculations")
                .and_then(Json::as_array)
                .expect("sources")
                .len(),
            2
        );
    }

    #[test]
    fn deck_requires_complete_dated_prices_and_a_comparable_source_basis() {
        for (change, expected) in [
            (
                DECK.replace(
                    "C-4 row 4\"\nknown_on = \"2030-04-12",
                    "C-4 row 4\"\nknown_on = \"2030-04-16",
                ),
                "unavailable on the decision date",
            ),
            (
                DECK.replace("element = \"Au\"", "element = \"Cu\""),
                "declared element",
            ),
            (
                DECK.replace("fixed_plan_flat_royalty_pre_tax", "progressive_royalties"),
                "full revaluation",
            ),
            (
                DECK.replace("usd_per_kg = \"70000\"", "usd_per_kg = \"-1\""),
                "nonnegative",
            ),
            (
                DECK.replace("state = \"hard\"", "state = \"unmapped\""),
                "exactly one committee price",
            ),
            (
                DECK.replace("state = \"hard\"", "state = \"soft\""),
                "unique",
            ),
            (
                DECK.split("\n[[price]]")
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n[[price]]"),
                "exactly one committee price",
            ),
        ] {
            let refusal = execute(&change).expect_err("incompatible input");
            assert!(refusal.what.contains(expected), "{}", refusal.what);
        }
    }
}
