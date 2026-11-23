//! DESIGN: source-bound investment, discount and complete period-bundle declarations; no filesystem IO.
//! Determinism: Exact

use super::*;
use crate::valuation::worksheet::{fields, get, invalid as parse_error, number};
use fmine_registry::toml_subset::parse_subset;

pub const SCHEMA: &str = "fmine.production-investment.v1";
pub const MAX_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSource {
    pub alternative: String,
    pub period: String,
    pub input: String,
    pub grinding: String,
    pub recovery: String,
    pub terms: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    declaration: Claim<InvestmentSpec>,
    sources: Vec<BundleSource>,
    text: String,
}

impl Manifest {
    pub fn declaration(&self) -> &Claim<InvestmentSpec> {
        &self.declaration
    }
    pub fn sources(&self) -> &[BundleSource] {
        &self.sources
    }
    /// Evaluate the already loaded bundles and retain the original manifest.
    ///
    /// The caller must resolve and load every path in [`Self::sources`] against
    /// its declared workspace root, then supply those exact evaluated receipts.
    /// This pure API checks receipt identities and economic/support compatibility;
    /// it does not resolve paths or establish that a receipt came from a named
    /// manifest path. The CLI owns that source-resolution step. Relative manifest
    /// paths and absolute source anchors are not compared by suffix or guessed.
    pub fn evaluate(&self, bundles: &[PeriodValuation]) -> Result<InvestmentReceipt> {
        let mut receipt = super::evaluate(&self.declaration, bundles)?;
        receipt.submitted_manifest = Some(self.text.clone());
        Ok(receipt)
    }
}

/// Paths are read by the CLI relative to its workspace root. Every economic
/// assumption, original token and source mapping remains bound to these exact bytes.
pub fn parse(text: &str, source: &SourceAnchor) -> Result<Manifest> {
    if text.len() as u64 > MAX_BYTES || !named(&source.artifact_path) || !named(&source.locator) {
        return Err(parse_error(
            1,
            "manifest at most 64 KiB with source path and locator required",
        ));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "investment source SHA-256 mismatch",
        ));
    }
    let doc = parse_subset(text).map_err(|e| parse_error(e.line, e.reason))?;
    let information_row =
        |table: &str| matches!(table, "information_option" | "signal" | "likelihood");
    if doc
        .rows
        .iter()
        .filter(|row| !information_row(&row.table))
        .count()
        > MAX_ALTERNATIVES + MAX_PERIODS + MAX_ALTERNATIVES * MAX_PERIODS + 17
    {
        return Err(parse_error(
            1,
            "too many investment, period, bundle or UU rows",
        ));
    }
    let mut required = vec![
        "schema",
        "model_key",
        "question",
        "shared_scenario_basis",
        "discount_basis",
        "cashflow_basis",
        "currency",
        "valuation_time_ns",
        "boundary",
        "circuit",
        "material_domain",
        "dry_basis",
        "element",
        "commercial_support",
        "monetary_rounding",
    ];
    if doc.top.iter().any(|f| f.key == "information_basis") {
        required.push("information_basis");
    }
    let top = fields(&doc.top, &required, 1)?;
    if get(&top, "schema")?.0 != SCHEMA
        || get(&top, "currency")?.0 != "USD"
        || get(&top, "monetary_rounding")?.0 != MonetaryRounding::NearestTiesEvenFourPlaces.as_str()
    {
        return Err(parse_error(
            1,
            "fmine.production-investment.v1, USD and nearest_ties_even_0_0001_usd required",
        ));
    }
    let mut anchors = vec![source.clone()];
    let (mut periods, mut alternatives, mut sources, mut unknowns) =
        (Vec::new(), Vec::new(), BTreeMap::new(), BTreeMap::new());
    let mut downside_limit = None;
    let time = |f: &crate::valuation::worksheet::Fields<'_>, key| {
        let (v, line) = get(f, key)?;
        v.parse::<i64>()
            .map_err(|_| parse_error(line, "time must be an i64 nanosecond integer"))
    };
    for row in &doc.rows {
        match row.table.as_str() {
            "period" => {
                let f = fields(
                    &row.fields,
                    &["id", "start_ns", "end_ns", "discount_factor"],
                    row.line,
                )?;
                periods.push(InvestmentPeriod {
                    id: get(&f, "id")?.0.into(),
                    start_ns: time(&f, "start_ns")?,
                    end_ns: time(&f, "end_ns")?,
                    discount_factor: number(&f, "discount_factor")?,
                });
            }
            "alternative" => {
                let f = fields(&row.fields, &["id", "label", "initial_capex_usd"], row.line)?;
                alternatives.push(InvestmentAlternative {
                    id: get(&f, "id")?.0.into(),
                    label: get(&f, "label")?.0.into(),
                    initial_capex_usd: number(&f, "initial_capex_usd")?,
                });
            }
            "bundle" => {
                let f = fields(
                    &row.fields,
                    &[
                        "alternative",
                        "period",
                        "input",
                        "grinding",
                        "recovery",
                        "terms",
                    ],
                    row.line,
                )?;
                let b = BundleSource {
                    alternative: get(&f, "alternative")?.0.into(),
                    period: get(&f, "period")?.0.into(),
                    input: get(&f, "input")?.0.into(),
                    grinding: get(&f, "grinding")?.0.into(),
                    recovery: get(&f, "recovery")?.0.into(),
                    terms: get(&f, "terms")?.0.into(),
                };
                if sources
                    .insert((b.alternative.clone(), b.period.clone()), b)
                    .is_some()
                {
                    return Err(parse_error(row.line, "duplicate investment-period bundle"));
                }
            }
            "downside_limit" => {
                if downside_limit.is_some() {
                    return Err(parse_error(
                        row.line,
                        "at most one downside limit is supported",
                    ));
                }
                let f = fields(
                    &row.fields,
                    &["floor_usd", "max_breach_probability"],
                    row.line,
                )?;
                let minor = number(&f, "floor_usd")?
                    .checked_mul(Rat::from_integer(Money::SCALE))
                    .map_err(|_| parse_error(row.line, "downside floor exceeds Money range"))?;
                if minor.den() != 1 {
                    return Err(parse_error(
                        get(&f, "floor_usd")?.1,
                        "downside floor must be exactly representable in 0.0001 USD; no rounding is applied",
                    ));
                }
                let maximum = number(&f, "max_breach_probability")?;
                if maximum.num() < 0 || maximum.num() > maximum.den() {
                    return Err(parse_error(
                        get(&f, "max_breach_probability")?.1,
                        "maximum breach probability must lie in [0,1]",
                    ));
                }
                downside_limit = Some(DownsideLimit {
                    model_key: get(&top, "model_key")?.0.into(),
                    valuation_basis: VALUATION_BASIS.into(),
                    floor: Money::from_minor(Currency::USD, minor.num()),
                    max_breach_probability: maximum,
                    policy_optimization: None,
                });
            }
            "uu" => {
                let f = fields(
                    &row.fields,
                    &["id", "description", "trigger", "downside_usd_m"],
                    row.line,
                )?;
                let exposure = number(&f, "downside_usd_m")?.to_f64();
                let id = get(&f, "id")?.0.to_string();
                if !exposure.is_finite()
                    || exposure <= 0.0
                    || unknowns
                        .insert(
                            id.clone(),
                            UuTerm {
                                scenario_id: id,
                                description: get(&f, "description")?.0.into(),
                                trigger_indicator: get(&f, "trigger")?.0.into(),
                                downside_exposure_usd_m: exposure,
                            },
                        )
                        .is_some()
                    || unknowns.len() > 16
                {
                    return Err(parse_error(
                        row.line,
                        "one to sixteen unique UU terms with positive finite exposure required",
                    ));
                }
            }
            "information_option" | "signal" | "likelihood" => {}
            _ => {
                return Err(parse_error(
                    row.line,
                    "only period, alternative, bundle, downside_limit, uu, information_option, signal and likelihood tables are supported",
                ));
            }
        }
        for field in &row.fields {
            anchors.push(SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {}: {}.{}", field.line, row.table, field.key),
            ));
        }
    }
    for field in &doc.top {
        anchors.push(SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            format!("line {}: {}", field.line, field.key),
        ));
    }
    let spec = InvestmentSpec {
        model_key: get(&top, "model_key")?.0.into(),
        question: get(&top, "question")?.0.into(),
        shared_scenario_basis: get(&top, "shared_scenario_basis")?.0.into(),
        discount_basis: get(&top, "discount_basis")?.0.into(),
        cashflow_basis: get(&top, "cashflow_basis")?.0.into(),
        currency: Currency::USD,
        valuation_time_ns: time(&top, "valuation_time_ns")?,
        boundary: get(&top, "boundary")?.0.into(),
        circuit: get(&top, "circuit")?.0.into(),
        material_domain: get(&top, "material_domain")?.0.into(),
        dry_basis: get(&top, "dry_basis")?.0.into(),
        element: get(&top, "element")?.0.into(),
        commercial_support: get(&top, "commercial_support")?.0.into(),
        monetary_rounding: MonetaryRounding::NearestTiesEvenFourPlaces,
        downside_limit,
        information_basis: top.get("information_basis").map(|(s, _)| (*s).into()),
        information_options: fmine_decide::card::model::parse_information_options(
            &doc.rows
                .iter()
                .filter(|row| information_row(&row.table))
                .cloned()
                .collect::<Vec<_>>(),
            get(&top, "model_key")?.0,
            Currency::USD,
            VALUATION_BASIS,
        )?,
        periods,
        alternatives,
    };
    validate_spec(&spec)?;
    if unknowns.is_empty()
        || sources.len() != spec.periods.len() * spec.alternatives.len()
        || sources.values().any(|b| {
            !spec.periods.iter().any(|p| p.id == b.period)
                || !spec.alternatives.iter().any(|a| a.id == b.alternative)
        })
    {
        return Err(parse_error(
            1,
            "complete investment-period bundles and an investment UU declaration required",
        ));
    }
    let tag = spec.canonical_tag();
    let declaration = Claim::new(
        INPUT_METRIC,
        spec,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Known,
        Confidentiality::internal(),
        unknowns.into_values().collect(),
        Vec::new(),
        anchors,
    );
    Ok(Manifest {
        declaration,
        sources: sources.into_values().collect(),
        text: text.into(),
    })
}
