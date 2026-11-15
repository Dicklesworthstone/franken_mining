//! DESIGN (BD-345.S4): printed gold and copper cost numerators divided by explicitly based metal sales.
//! Declared identity bases never replace the original operands' printed and registered basis evidence.

use super::*;
use fmine_claim::Claim;
use fmine_core::anchor::SourceAnchor;
use fmine_metrics::public_derivation::{CostRateUnit, derive_cost_rate};
use fmine_numerics::exact_feas::Interval;
use fmine_public::LoadedSource;
use fmine_public::aisc::{self, IdentityFamily};
use fmine_public::sources::SourceKind;

const GOLD_COS_NUMERATOR: &str = "cost of sales applicable to gold production";

#[derive(Debug, Clone, Copy)]
struct RateKind {
    unit: CostRateUnit,
    family: &'static str,
    numerator: &'static str,
    rate_label: &'static str,
    basis: &'static str,
}

impl RateKind {
    const ALL: [Self; 6] = [
        Self {
            unit: CostRateUnit::GoldKoz,
            family: "aisc_per_oz",
            numerator: "all-in sustaining costs",
            rate_label: "all-in sustaining costs per ounce",
            basis: IdentityFamily::AiscPerOunce.basis(),
        },
        Self {
            unit: CostRateUnit::GoldKoz,
            family: "tcc_per_oz",
            numerator: "total cash costs",
            rate_label: "total cash costs per ounce",
            basis: IdentityFamily::TccPerOunce.basis(),
        },
        Self {
            unit: CostRateUnit::GoldKoz,
            family: "cos_per_oz",
            numerator: "site:costs ($ millions):cost of sales",
            rate_label: "site:costs per ounce:cost of sales",
            basis: "Cost of sales US$M over gold sales in thousands of ounces, both on the same explicitly printed ownership basis",
        },
        Self {
            unit: CostRateUnit::CopperMillionPounds,
            family: "aisc_per_lb",
            numerator: "site:costs ($ millions):all-in sustaining costs",
            rate_label: "site:costs per pound:all-in sustaining costs",
            basis: "AISC US$M over copper sales in millions of pounds, both on the same explicitly printed ownership basis",
        },
        Self {
            unit: CostRateUnit::CopperMillionPounds,
            family: "c1_per_lb",
            numerator: "site:costs ($ millions):c1 cash costs",
            rate_label: "site:costs per pound:c1 cash costs",
            basis: "C1 cash costs US$M over copper sales in millions of pounds, both on the same explicitly printed ownership basis",
        },
        Self {
            unit: CostRateUnit::CopperMillionPounds,
            family: "cos_per_lb",
            numerator: "site:costs ($ millions):cost of sales",
            rate_label: "site:costs per pound:cost of sales",
            basis: "Cost of sales US$M over copper sales in millions of pounds, both on the same explicitly printed ownership basis",
        },
    ];

    fn denominator(self) -> &'static str {
        match (self.unit, self.family) {
            (CostRateUnit::GoldKoz, "cos_per_oz") => "site::sales (thousands of ounces)",
            (CostRateUnit::GoldKoz, _) => "ounces sold",
            (CostRateUnit::CopperMillionPounds, _) => "site::sales (millions of pounds)",
        }
    }

    fn accepts_basis(self, cell: &KeyedCell) -> bool {
        explicit_basis(cell.key.ownership_basis).is_some()
            && (self.unit == CostRateUnit::CopperMillionPounds
                || self.family == "cos_per_oz"
                || matches!(
                    cell.key.ownership_basis,
                    OwnershipBasis::Attributable { .. }
                ))
    }

    fn is_numerator(self, cell: &KeyedCell) -> bool {
        cell.key.line.label == self.numerator
            || (self.family == "cos_per_oz" && cell.key.line.label == GOLD_COS_NUMERATOR)
    }

    fn is_denominator(self, cell: &KeyedCell) -> bool {
        cell.key.line.label == self.denominator()
            || (self.family == "cos_per_oz" && cell.key.line.label == "ounces sold")
    }
}

#[derive(Debug, Clone)]
pub(super) struct Assessment {
    pub(super) status: &'static str,
    reason: &'static str,
    kind: RateKind,
    pub(super) evidence: Vec<SourceEvidence>,
    draft: Option<Interval>,
    derived: Option<Claim<Interval>>,
    refusal: Option<Refusal>,
    basis_bridge: Option<BasisBridge>,
}

#[derive(Debug, Clone)]
struct BasisWitness {
    anchor: SourceAnchor,
    publication: Publication,
    url: String,
    text: String,
}

#[derive(Debug, Clone)]
struct BasisBridge {
    factor: Rat,
    rule: &'static str,
    witnesses: Vec<BasisWitness>,
    numerator: Claim<Printed>,
}

fn column(c: &KeyedCell) -> Option<(&str, u32)> {
    let (sheet, cell) = c.locator.rsplit_once('!')?;
    Some((sheet, fmine_io::xlsx::parse_a1_ref(cell)?.1))
}

pub(super) fn assess(
    draft: &str,
    f: &LintFinding,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
    sources: &[LoadedSource],
) -> Option<Assessment> {
    let (prototype, target, kind) = cells.iter().find_map(|sold| {
        RateKind::ALL.into_iter().find_map(|kind| {
            if !kind.is_denominator(sold)
                || sold.key.unit != kind.unit.denominator_unit()
                || !kind.accepts_basis(sold)
            {
                return None;
            }
            // A grammar prototype changes only metric/unit; it supplies neither a number nor a printed share.
            let mut p = sold.clone();
            p.key.line.label = kind.rate_label.into();
            p.key.unit = kind.unit.rate_unit().into();
            Some((sold, equality_target(draft, f.span, &p)?, kind))
        })
    })?;
    let mut evidence: Vec<_> = cells
        .iter()
        .filter(|c| {
            c.key.issuer == prototype.key.issuer
                && c.key.subject == prototype.key.subject
                && c.key.valid_period == target.period
                && (kind.is_numerator(c) || kind.is_denominator(c))
        })
        .map(|c| source_evidence(c, profile, publications, as_of))
        .collect();
    evidence.sort_by_cached_key(|e| (e.source.claim.claim_id, e.to_json()));
    evidence.dedup_by(|a, b| a.source == b.source);
    let mut out = Assessment {
        status: "not_available",
        reason: "unique_dated_same_source_cost_and_explicit_sales_basis_required",
        kind,
        evidence,
        draft: None,
        derived: None,
        refusal: None,
        basis_bridge: None,
    };
    let run = (|| -> Result<(), Refusal> {
        let interval = sums::draft_interval(target)?;
        out.draft = Some(interval);
        let (mut numerators, mut denominators): (Vec<_>, Vec<_>) = out
            .evidence
            .iter_mut()
            .filter(|e| e.admission != "after_cut")
            .partition(|e| kind.is_numerator(&e.source));
        if numerators.len() > 1 || denominators.len() > 1 {
            out.status = "ambiguous";
            out.reason = "competing_cost_or_sales_assertions_are_not_selected_by_amount";
            return Ok(());
        }
        let ([n], [q]) = (numerators.as_mut_slice(), denominators.as_mut_slice()) else {
            return Ok(());
        };
        if n.admission != "published_by_cut" || q.admission != "published_by_cut" {
            return Ok(());
        }
        n.relation = "cost_numerator";
        q.relation = "sold_denominator";
        let (n, q) = (n.source.clone(), q.source.clone());
        let reconciled_cos = n.key.line.label == GOLD_COS_NUMERATOR;
        let strict = kind.unit == CostRateUnit::CopperMillionPounds || kind.family == "cos_per_oz";
        if n.key.unit != "usd_m"
            || q.key.unit != kind.unit.denominator_unit()
            || n.key.reporting_vintage != q.key.reporting_vintage
            || !column(&n).is_some_and(|col| Some(col) == column(&q))
            || n.key.scope != q.key.scope
            || n.key.line.occurrence != q.key.line.occurrence
            || !kind.accepts_basis(&q)
            || (n.key.ownership_basis != OwnershipBasis::Unprinted
                && explicit_basis(n.key.ownership_basis) != explicit_basis(q.key.ownership_basis))
            || [&n, &q].iter().any(|c| {
                c.claim.additivity != AdditivityClass::ExtensiveAdditive
                    || (strict && !reconciled_cos && explicit_basis(c.printed_basis).is_none())
                    || (c.printed_basis != OwnershipBasis::Unprinted
                        && explicit_basis(c.printed_basis) != explicit_basis(c.key.ownership_basis))
                    || matches!(&c.key.scope, Scope::Region { map_version }
                    if map_version.is_empty() || map_version.starts_with("unregistered:"))
            })
        {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                "cost-rate operands must share source table, column, period, perimeter and an admissible explicit ownership basis",
            ));
        }
        let derived = if reconciled_cos {
            let bridge = aisc_numerator_basis(
                &n,
                &q,
                cells,
                profile,
                publications,
                as_of,
                sources,
                &mut out.evidence,
            )?;
            let derived = derive_cost_rate(
                kind.unit,
                kind.family,
                (&bridge.numerator, Some(bridge.numerator.value)),
                (&q.claim, printed(&q)),
            )?;
            out.basis_bridge = Some(bridge);
            derived
        } else {
            derive_cost_rate(
                kind.unit,
                kind.family,
                (&n.claim, printed(&n)),
                (&q.claim, printed(&q)),
            )?
        };
        out.status = sums::compare(interval, derived.value)?;
        out.reason = match out.status {
            "derived" => "same_source_cost_over_sales_supports_the_draft_precision",
            "contradicted" => "cost_rate_enclosure_is_disjoint_from_the_draft",
            _ => "cost_and_sales_rounding_exceeds_the_draft_precision",
        };
        out.derived = Some(derived);
        Ok(())
    })();
    if let Err(refusal) = run {
        out.status = "refused";
        out.reason = "source_interpretation_or_exact_cost_rate_failed";
        out.refusal = Some(refusal);
    }
    Some(out)
}

/// DESIGN: an operating NCI line selects the issuer's stated cost-of-sales numerator rule.
/// It never changes either source Claim's ownership label. Sustaining NCI has no role in this rule.
#[allow(clippy::too_many_arguments)]
fn aisc_numerator_basis(
    n: &KeyedCell,
    q: &KeyedCell,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
    sources: &[LoadedSource],
    evidence: &mut Vec<SourceEvidence>,
) -> Result<BasisBridge, Refusal> {
    let bad = |what: &str| Refusal::new(RefusalCode::BasisMismatch105, what);
    if !matches!(n.key.scope, Scope::Site | Scope::JointVenture)
        || fmine_public::key::parse_period_code(&n.key.reporting_vintage.release)
            != Some(n.key.valid_period)
    {
        return Err(bad(
            "gold AISC cost of sales needs a site and the original quarterly period; mixed regions and comparative ownership are not inferred",
        ));
    }
    let registered = source_for(n, profile)
        .ok_or_else(|| bad("gold cost numerator has no unique registered source"))?;
    let matching: Vec<_> = sources
        .iter()
        .filter(|source| {
            source.file_name == registered.file_name
                && source.kind == SourceKind::AiscReconciliation
                && source.quarter == registered.quarter
                && Digest32(source.workbook.sha256).to_hex() == registered.sha256_hex
        })
        .collect();
    let [source] = matching.as_slice() else {
        return Err(bad(
            "gold AISC cost of sales requires its verified reconciliation workbook",
        ));
    };
    let (sheet_name, n_ref) = n
        .locator
        .strip_prefix(&format!("{}:", source.file_name))
        .and_then(|s| s.rsplit_once('!'))
        .ok_or_else(|| bad("invalid gold cost numerator locator"))?;
    let sheet = source
        .workbook
        .sheets
        .iter()
        .find(|s| s.name == sheet_name)
        .ok_or_else(|| bad("gold cost numerator requires a visible source sheet"))?;
    let table = aisc::locate_table(sheet, &profile.subjects)
        .map_err(|why| bad(&why))?
        .ok_or_else(|| bad("gold cost source has no anchored AISC table"))?;
    let (row, col) = fmine_io::xlsx::parse_a1_ref(n_ref)
        .ok_or_else(|| bad("invalid gold cost numerator coordinate"))?;
    if row != table.cos_row
        || q.locator
            != format!(
                "{}:{}!{}{}",
                source.file_name,
                sheet.name,
                fmine_io::xlsx::col_index_to_letters(col),
                table.ounces_row
            )
        || !aisc::is_usd_millions_label(
            &aisc::cell_text(sheet, 3, 1)
                .unwrap_or_default()
                .to_lowercase(),
        )
    {
        return Err(bad(
            "gold cost numerator and attributable koz must be the anchored table's own US$M and ounces rows",
        ));
    }
    let basis = aisc::bridge_basis(
        profile,
        sources,
        source,
        sheet,
        &table,
        &n.key.subject,
        q.key.ownership_basis,
    )
    .map_err(|why| bad(&why))?;
    let mut witnesses = basis.evidence;
    witnesses.push(format!("{}:{}!A3", source.file_name, sheet.name));
    let mut inputs = vec![n, q];
    let mut nonzero_nci = false;
    for &row in table.nci_rows.iter().filter(|r| table.block1.contains(r)) {
        let locator = format!(
            "{}:{}!{}{}",
            source.file_name,
            sheet.name,
            fmine_io::xlsx::col_index_to_letters(col),
            row
        );
        let matches: Vec<_> = cells
            .iter()
            .filter(|c| {
                c.locator == locator
                    && c.key.reporting_vintage == n.key.reporting_vintage
                    && c.key.subject == n.key.subject
                    && c.key.valid_period == n.key.valid_period
                    && c.key.scope == n.key.scope
                    && c.key.unit == "usd_m"
            })
            .collect();
        let [nci] = matches.as_slice() else {
            return Err(bad(
                "operating NCI requires one canonical same-table source Claim",
            ));
        };
        let mut item = source_evidence(nci, profile, publications, as_of);
        item.relation = "operating_nci_basis_witness";
        let admitted = item.admission == "published_by_cut";
        evidence.push(item);
        let amount = printed(nci)
            .filter(|_| admitted)
            .ok_or_else(|| bad("operating NCI must be a dated, usable displayed number"))?;
        nonzero_nci |= amount.value.signum() != 0;
        inputs.push(nci);
    }
    if inputs.len() == 2 {
        return Err(bad(
            "the operating NCI basis witness is missing; a zero is not inferred",
        ));
    }
    let (factor, rule) = if nonzero_nci {
        let footnote = aisc::find_share_footnote(sheet)
            .ok_or_else(|| bad("the attributable cost-of-sales formula footnote is missing"))?;
        // The shared locator returns the first match. Distinct matching statements cannot choose
        // an ownership interpretation by cell order; identical repeated notes add no ambiguity.
        let statements: BTreeSet<_> = sheet
            .cells_by_coord
            .keys()
            .filter_map(|&(row, col)| aisc::cell_text(sheet, row, col))
            .map(|text| text.to_lowercase())
            .filter(|text| {
                [
                    "is calculated as cost of sales",
                    "divided by ounces sold",
                    "attributable basis using",
                    "ownership share",
                ]
                .iter()
                .all(|phrase| text.contains(phrase))
            })
            .collect();
        if statements.len() > 1 {
            return Err(bad(
                "competing cost-of-sales formula footnotes leave the ownership basis ambiguous",
            ));
        }
        witnesses.push(format!("{}:{}!{footnote}", source.file_name, sheet.name));
        let OwnershipBasis::Attributable { num, den, .. } = basis.basis else {
            return Err(bad(
                "the nonzero operating NCI bridge requires an explicit attributable share",
            ));
        };
        (
            Rat::new(i128::from(num), i128::from(den))
                .map_err(|_| bad("invalid ownership share"))?,
            "nonzero_operating_nci_apply_confirmed_share",
        )
    } else {
        (
            Rat::from_integer(1),
            "zero_operating_nci_preserve_printed_numerator",
        )
    };
    let witnesses = witnesses
        .into_iter()
        .map(|at| basis_witness(&at, sources, profile, publications, as_of))
        .collect::<Result<Vec<_>, _>>()?;
    let raw = printed(n).ok_or_else(|| bad("cost numerator has no displayed precision"))?;
    let value = Printed {
        value: raw
            .value
            .checked_mul(factor)
            .map_err(|_| bad("scaled numerator overflows"))?,
        step: raw
            .step
            .checked_mul(factor)
            .map_err(|_| bad("scaled numerator precision overflows"))?,
    };
    let mut terms = Vec::new();
    let mut anchors = Vec::new();
    let mut parents = Vec::new();
    let mut evidence_class = EvidenceClass::Derived;
    let mut epistemic = n.claim.epistemic;
    let mut state = n.claim.state;
    let mut confidentiality = n.claim.confidentiality.clone();
    for c in inputs {
        for term in &c.claim.uu_terms {
            if !terms.contains(term) {
                terms.push(term.clone());
            }
        }
        anchors.extend(c.claim.anchors.iter().cloned());
        parents.push(c.claim.claim_id);
        evidence_class = evidence_class.meet(c.claim.evidence);
        epistemic = epistemic.and(c.claim.epistemic);
        state = state.and(c.claim.state);
        confidentiality = confidentiality.join(&c.claim.confidentiality);
    }
    anchors.extend(witnesses.iter().map(|w| w.anchor.clone()));
    anchors.sort();
    anchors.dedup();
    parents.sort();
    parents.dedup();
    let tag = format!(
        "{rule}:factor={}/{};value={}/{};step={}/{}",
        factor.num(),
        factor.den(),
        value.value.num(),
        value.value.den(),
        value.step.num(),
        value.step.den()
    );
    let numerator = Claim::new(
        "public-disclosure.gold-aisc-cos-numerator/1",
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        evidence_class,
        epistemic,
        state,
        confidentiality,
        terms,
        parents,
        anchors,
    );
    Ok(BasisBridge {
        factor,
        rule,
        witnesses,
        numerator,
    })
}

fn basis_witness(
    at: &str,
    sources: &[LoadedSource],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
) -> Result<BasisWitness, Refusal> {
    let bad = || {
        Refusal::new(
            RefusalCode::NotObservable202,
            format!(
                "gold cost basis witness {at} needs exact registered source bytes and a publication date at or before {as_of}"
            ),
        )
    };
    let (file, locator) = at.split_once(':').ok_or_else(bad)?;
    let source = sources
        .iter()
        .find(|s| s.file_name == file)
        .ok_or_else(bad)?;
    let hash = Digest32(source.workbook.sha256).to_hex();
    let registered = profile
        .sources
        .iter()
        .find(|s| s.file_name == file && s.sha256_hex == hash && s.quarter == source.quarter)
        .ok_or_else(bad)?;
    let publication = publications
        .get(&(file.to_string(), hash))
        .copied()
        .filter(|p| p.published_at.as_str() <= as_of)
        .ok_or_else(bad)?;
    let (sheet_name, cell) = locator.rsplit_once('!').ok_or_else(bad)?;
    let (row, col) = fmine_io::xlsx::parse_a1_ref(cell).ok_or_else(bad)?;
    let sheet = source
        .workbook
        .sheets
        .iter()
        .find(|s| s.name == sheet_name)
        .ok_or_else(bad)?;
    let text = aisc::cell_text(sheet, row, col)
        .filter(|s| !s.is_empty())
        .ok_or_else(bad)?;
    Ok(BasisWitness {
        anchor: SourceAnchor::new(
            &source.workbook.artifact_path,
            source.workbook.sha256,
            locator,
        ),
        publication: publication.clone(),
        url: registered.url.clone(),
        text,
    })
}

impl Assessment {
    fn formula(&self) -> &'static str {
        if self.basis_bridge.is_some() {
            "1000*f*N/Q"
        } else {
            self.kind.unit.formula()
        }
    }

    fn basis(&self) -> &'static str {
        if self.basis_bridge.is_some() {
            "Gold AISC reconciliation: f is the same-release label-confirmed ownership share for nonzero operating NCI, or one when the operating NCI is explicitly zero; sustaining NCI never changes f"
        } else {
            self.kind.basis
        }
    }

    pub(super) fn to_json(&self) -> String {
        let derived = self.derived.as_ref().map_or_else(
            || "null".into(),
            |c| {
                canonical_json(
                    c,
                    &format!("derived_cost_rate_{}", self.kind.unit.rate_unit()),
                    sums::interval(c.value),
                )
            },
        );
        format!(
            "{{\"rule\":{},\"family\":{},\"identity_basis\":{},\"formula\":{},\"unit\":{},\"construction\":\"each printed operand plus or minus half its own displayed step\",\"status\":{},\"reason\":{},\"warning\":{},\"draft_interval\":{},\"derived_claim\":{},\"evidence\":[{}],\"refusal\":{}{}}}",
            js(self.kind.unit.rule()),
            js(self.kind.family),
            js(self.basis()),
            js(self.formula()),
            js(self.kind.unit.display_unit()),
            js(self.status),
            js(self.reason),
            js(fmine_metrics::public_derivation::WARNING),
            self.draft.map_or_else(|| "null".into(), sums::interval),
            derived,
            evidence_json(&self.evidence),
            self.refusal.as_ref().map_or_else(
                || "null".into(),
                |r| format!("{{\"code\":{},\"what\":{}}}", js(r.code), js(&r.what))
            ),
            self.basis_bridge
                .as_ref()
                .map_or_else(String::new, |basis| {
                    format!(",\"basis_bridge\":{}", basis.to_json())
                }),
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "  - Cost rate `{}` ({}), {} in {}: **{}**; {}. {}. {}\n",
            self.kind.unit.rule(),
            self.kind.family,
            self.formula(),
            self.kind.unit.display_unit(),
            self.status,
            self.reason,
            self.basis(),
            fmine_metrics::public_derivation::WARNING
        );
        if let Some(c) = &self.derived {
            sums::derived_markdown(&mut out, c.claim_id, c.value);
        }
        if let Some(basis) = &self.basis_bridge {
            let _ = writeln!(
                out,
                "    Numerator factor {}/{}: {}; interpreted numerator Claim {}.",
                basis.factor.num(),
                basis.factor.den(),
                basis.rule,
                basis.numerator.claim_id
            );
            for w in &basis.witnesses {
                let _ = writeln!(
                    out,
                    "    Basis witness {}:{}; SHA-256 {}; published {} ({}).",
                    w.publication.file,
                    w.anchor.locator,
                    w.publication.sha256,
                    w.publication.published_at,
                    w.publication.published_at_basis
                );
            }
        }
        out
    }
}

impl BasisBridge {
    fn to_json(&self) -> String {
        let witnesses = self.witnesses.iter().map(|w| format!(
            "{{\"file\":{},\"locator\":{},\"sha256\":{},\"url\":{},\"published_at\":{},\"published_at_basis\":{},\"text\":{}}}",
            js(&w.publication.file), js(&w.anchor.locator), js(&w.publication.sha256),
            js(&w.url), js(&w.publication.published_at), js(&w.publication.published_at_basis), js(&w.text)
        )).collect::<Vec<_>>().join(",");
        let exact = |v: Rat| js(&format!("{}/{}", v.num(), v.den()));
        let numerator = canonical_json(
            &self.numerator,
            "interpreted_gold_cost_numerator_usd_m",
            format!(
                "{{\"value\":{},\"step\":{}}}",
                exact(self.numerator.value.value),
                exact(self.numerator.value.step)
            ),
        );
        format!(
            "{{\"rule\":{},\"factor\":{},\"interpreted_numerator\":{},\"witnesses\":[{}]}}",
            js(self.rule),
            exact(self.factor),
            numerator,
            witnesses
        )
    }
}
