//! DESIGN (bd-t773): an exact accounting bridge between two replayed plant outcomes.

use fmine_calib::{OutcomeCitation, OutcomeKind};
use fmine_core::{Digest32, ObjectId};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty, UnitsError};

use super::super::{Replay, absent, js, utc_text};
use super::outcome::{BalanceMasses, TargetScope, target_scope};

pub(in crate::forecast) const NO_CLAIM: &str = "Arithmetic attribution between declared, source-derived plant observations. Replay checks retained balance bytes and exact closure. Measurement accuracy, stream mapping, availability and completeness remain supplied and unverified. The contributions share the interaction terms symmetrically and establish no causal effect, calibration or operating authority.";
pub(super) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal explain-plant-change --journal <path> --head <hex> --baseline <earlier-forecast> --current <later-forecast>","input":"Two source-resolved recovered-metal outcomes from the pinned history, with positive dry feed and feed element mass. Use the same boundary, circuit, dry basis and element, in ordered nonoverlapping UTC intervals of equal duration. Retains each observed blend and material domain","method":"Exact change in recovered kg, split across dry tonnes, head grade and recovery by averaging all six factor orders equally","evidence":"Includes both outcome citations and retained period quantities. Takes no clock sample and reads no editable sources. Arithmetic attribution only, with observation authenticity and causality unestablished"}"#;

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
const THOUSAND: ExactQty = ExactQty::from_integer(1_000);

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "plant change attribution exceeds exact rational arithmetic",
        )
    })
}

struct Period<'a> {
    id: ObjectId,
    alias: &'a str,
    scope: &'a TargetScope,
    citation: &'a OutcomeCitation,
    recovered: ExactQty,
    // Dry metric tonnes, kg of the element per dry tonne, recovered fraction.
    factors: [ExactQty; 3],
}

fn period<'a>(state: &'a Replay, alias: &'a str) -> Result<Period<'a>> {
    let id = state.forecast_id(alias)?;
    let unsupported = || {
        absent(format!(
            "{alias}: a single source-resolved recovered-metal outcome is required"
        ))
    };
    let [event] = state.ledger.outcomes(id) else {
        return Err(unsupported());
    };
    let OutcomeKind::Published { citation, .. } = &event.kind else {
        return Err(unsupported());
    };
    let BalanceMasses {
        dry_feed_kg,
        feed_element_kg,
        recovered_element_kg,
    } = state
        .plant_outcome_admissions
        .get(&(id, event.seq))
        .and_then(|admission| admission.masses)
        .ok_or_else(unsupported)?;
    if dry_feed_kg.signum() <= 0 || feed_element_kg.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "{alias}: grade and recovery require positive dry feed and positive feed element mass"
            ),
        ));
    }
    let tonnes = exact(dry_feed_kg.checked_div(THOUSAND))?;
    Ok(Period {
        id,
        alias,
        scope: target_scope(state, id)?,
        citation,
        recovered: recovered_element_kg,
        factors: [
            tonnes,
            exact(feed_element_kg.checked_div(tonnes))?,
            exact(recovered_element_kg.checked_div(feed_element_kg))?,
        ],
    })
}

fn signed_effects(
    before: [ExactQty; 3],
    after: [ExactQty; 3],
) -> std::result::Result<[ExactQty; 3], UnitsError> {
    let mut effects = [ZERO; 3];
    let two = ExactQty::from_integer(2);
    let six = ExactQty::from_integer(6);
    for (i, (j, k)) in [(1, 2), (0, 2), (0, 1)].into_iter().enumerate() {
        // The driver changes first in two orders, last in two, and once after each other driver.
        let weight = before[j]
            .checked_mul(before[k])?
            .checked_mul(two)?
            .checked_add(before[j].checked_mul(after[k])?)?
            .checked_add(after[j].checked_mul(before[k])?)?
            .checked_add(after[j].checked_mul(after[k])?.checked_mul(two)?)?
            .checked_div(six)?;
        effects[i] = after[i].checked_sub(before[i])?.checked_mul(weight)?;
    }
    Ok(effects)
}

fn ratio(q: ExactQty) -> String {
    if q.den() == 1 {
        q.num().to_string()
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

impl Period<'_> {
    fn facts(&self) -> Result<(String, String)> {
        let [tonnes, grade_kg_per_t, recovery] = self.factors;
        let grade = exact(grade_kg_per_t.checked_mul(THOUSAND))?;
        let scope = self.scope;
        let start = utc_text(scope.start_ns);
        let end = utc_text(scope.end_ns);
        let digest = Digest32(self.citation.sha256);
        let json = format!(
            r#"{{"alias":{},"forecast_id":{},"start_utc":{},"end_utc":{},"blend":{},"material_domain":{},"dry_tonnes":{},"head_grade_g_per_dry_t":{},"recovery_fraction":{},"recovered_kg":{},"source":{},"vintage":{},"locator":{},"evidence_sha256":{}}}"#,
            js(self.alias),
            js(&self.id.to_string()),
            js(&start),
            js(&end),
            js(&scope.blend),
            js(&scope.material_domain),
            js(&ratio(tonnes)),
            js(&ratio(grade)),
            js(&ratio(recovery)),
            js(&ratio(self.recovered)),
            js(&self.citation.source),
            js(&self.citation.vintage),
            js(&self.citation.locator),
            js(&digest.to_hex()),
        );
        let markdown = format!(
            "| {} | {} to {} | {} / {} | {} | {} | {} | {} | `{digest}` |",
            crate::guidance::text(self.alias),
            start,
            end,
            crate::guidance::text(&scope.blend),
            crate::guidance::text(&scope.material_domain),
            ratio(tonnes),
            ratio(grade),
            ratio(recovery),
            ratio(self.recovered),
        );
        Ok((json, markdown))
    }
}

pub(in crate::forecast) fn report(
    state: &Replay,
    head: Digest32,
    baseline: &str,
    current: &str,
) -> Result<(String, String)> {
    let baseline = period(state, baseline)?;
    let current = period(state, current)?;
    let old = baseline.scope;
    let new = current.scope;
    let duration = |scope: &TargetScope| i128::from(scope.end_ns) - i128::from(scope.start_ns);
    if old.boundary != new.boundary
        || old.circuit != new.circuit
        || old.dry_basis != new.dry_basis
        || old.element != new.element
        || old.end_ns > new.start_ns
        || duration(old) != duration(new)
    {
        return Err(Refusal::new(
            RefusalCode::SupportMismatch102,
            "plant change needs the same boundary, circuit, dry basis and element in ordered, nonoverlapping periods of equal UTC duration",
        ));
    }
    let [tonnes, grade, recovery] = exact(signed_effects(baseline.factors, current.factors))?;
    let delta = exact(current.recovered.checked_sub(baseline.recovered))?;
    if exact(
        tonnes
            .checked_add(grade)
            .and_then(|sum| sum.checked_add(recovery)),
    )? != delta
    {
        return Err(Refusal::new(
            RefusalCode::Conservation101,
            "plant driver contributions must sum exactly to the observed change",
        ));
    }
    let (old_json, old_row) = baseline.facts()?;
    let (new_json, new_row) = current.facts()?;
    let payload = format!(
        r#"{{"schema":"fmine.plant-change.v1","head":{},"head_pinned":true,"boundary":{},"circuit":{},"dry_basis":{},"element":{},"baseline":{old_json},"current":{new_json},"change_kg":{},"effects_kg":{{"dry_tonnes":{},"head_grade":{},"recovery":{}}},"interaction_allocation":"mean_of_six_factor_orders","source_class":"source_derived_unverified"}}"#,
        js(&head.to_hex()),
        js(&old.boundary),
        js(&old.circuit),
        js(&old.dry_basis),
        js(&old.element),
        js(&ratio(delta)),
        js(&ratio(tonnes)),
        js(&ratio(grade)),
        js(&ratio(recovery)),
    );
    let markdown = format!(
        "# Recovered-metal change\n\n{} at {} / {}. Dry basis `{}`. Pinned journal head `{head}`.\n\n| Period | UTC support | Blend / material domain | Dry tonnes | Grade (g/dry t) | Recovery fraction | Recovered kg | Evidence SHA-256 |\n|---|---|---|---:|---:|---:|---:|---|\n{old_row}\n{new_row}\n\n| Driver | Change in recovered kg |\n|---|---:|\n| Dry tonnes | {} |\n| Head grade | {} |\n| Recovery | {} |\n| **Total change** | **{}** |\n\n{NO_CLAIM}\n",
        crate::guidance::text(&old.element),
        crate::guidance::text(&old.boundary),
        crate::guidance::text(&old.circuit),
        crate::guidance::text(&old.dry_basis),
        ratio(tonnes),
        ratio(grade),
        ratio(recovery),
        ratio(delta),
    );
    Ok((payload, markdown))
}
