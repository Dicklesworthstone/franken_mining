//! DESIGN (BD-181/263): derive plant outcomes from complete stock-neutral material observations.
//! No prediction or assay-implied split becomes an observation. Every original worksheet remains
//! in the bounded journal operation.
//! Determinism: Exact source arithmetic with an explicit binary64 scoring projection.

use std::path::{Path, PathBuf};

use fmine_calib::{OutcomeCitation, OutcomeKind};
use fmine_claim::forecast_journal::MAX_OPERATION_BYTES;
use fmine_core::{Digest32, ObjectId, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{Json, parse};
use fmine_plant::measurements::two_product;
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_units::ExactQty;

use super::super::{Replay, Row, absent, hex, identifier, invalid, js, time};
use super::{Target, registration_start, retained_field, retained_text};

pub(in crate::forecast) const RULE: &str = "plant-two-product-recovery/v1";
const RECOVERY_SCHEMA: &str = "fmine.plant-recovery-observation.v1";
const PRODUCTION_SCHEMA: &str = "fmine.plant-production-observation.v1";
const PRODUCTION_RULE: &str = "plant-two-product-production/v1";
const BASIS: &str = "complete-two-product/no-inventory-change";
const DRY_BASIS: &str = "total-non-water-dry-matter/v1";
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);

const RECOVERY_NO_CLAIM: &str = "Recovery is the exact concentrate/feed total-element mass ratio from declared whole-material observations. Complete feed, concentrate and tailings, unchanged circuit inventory and exact dry/elemental closure are required. Source bytes and arithmetic are checked, but measurement accuracy, stream mapping, availability and completeness remain supplied and unverified. No assay-implied, reconciled or modelled value is substituted. The exact ratio and its binary64 scoring projection are retained; no calibration, P754 compliance, external time proof or operating authority is established.";
pub(in crate::forecast) const NO_CLAIM: &str = "Plant outcomes come from declared whole-material observations with unchanged inventory and exact dry and selected-element closure. Source bytes and arithmetic are checked. Measurement accuracy, stream mapping, completeness and availability remain supplied and unverified. Scoring uses the retained binary64 projection of the exact amount. This receipt grants no operating authority or calibration status.";

pub(in crate::forecast) const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"command":"forecast journal resolve-plant --journal <path> --head <hex> --cohort <name> --forecast <alias> --input <observation.toml> --trust-host-clock","#,
    r#""target":"Registered plant recovery, dry-tonnes, contained-metal or recovered-metal in v14 or legacy v9. The frozen forecast selects the quantity. Operating-capacity throughput requires different evidence","#,
    r#""schema":"fmine.plant-recovery-observation.v1","#,
    r#""recovery":{"schema":"fmine.plant-recovery-observation.v1","resolution_rule":"plant-two-product-recovery/v1","calculation":"Concentrate metal kg divided by positive feed metal kg","operation":"v16"},"#,
    r#""production":{"schema":"fmine.plant-production-observation.v1","resolution_rule":"plant-two-product-production/v1","calculation":"Dry feed kg divided by 1000 for metric tonnes, feed metal kg for contained metal, and concentrate metal kg for recovered metal","operation":"v17"},"#,
    r#""cohort":"Freeze membership before the physical interval starts with the matching resolution_rule and observation source among eligible_sources","#,
    r#""observation_fields":["source","vintage","available_at","circuit","blend","material_domain","dry_basis","element","measurand","balance_basis","feed","concentrate","tailings"],"#,
    r#""layout":"One schema field and one [[observation]] table; values are quoted. Relative material paths resolve against the descriptor directory","#,
    r#""basis":"balance_basis = complete-two-product/no-inventory-change; dry_basis = total-non-water-dry-matter/v1; measurand = total-<element>/all-phases","#,
    r#""sources":"Three fmine.plant-material.v1 whole-material worksheets with distinct stream identities. Boundary and exact UTC interval must match the frozen forecast. Circuit, blend, material domain, dry basis and element must match","#,
    r#""calculation":"Require exact dry and selected-element closure and use the original stream amounts without fitted values or tolerances","#,
    r#""time":"Declared availability must strictly follow physical end and be at or before the earliest writer clock bound","#,
    r#""journal":"The descriptor, source worksheets and SHA-256, exact amount, binary64 scoring projection and diagnostic receipt must all fit in 64 KiB. Earlier operation semantics are preserved","#,
    r#""replay":"Recompute from the retained observations with exact dry and selected-element closure. Admission refuses duplicate outcomes and missing evidence atomically","#,
    r#""interpretation":"source_derived_unverified. Accuracy, stream mapping, completeness and availability remain source declarations"}"#,
);

type Result<T> = std::result::Result<T, Refusal>;

fn fail(code: RefusalCode, what: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant outcome: {}", what.into()))
}

fn changed(what: impl Into<String>) -> Refusal {
    fail(RefusalCode::DigestMismatch505, what)
}

fn exact<T>(result: std::result::Result<T, fmine_units::UnitsError>) -> Result<T> {
    result.map_err(|_| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "exact observation arithmetic is unrepresentable",
        )
    })
}

fn label(value: &str) -> Result<String> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > 4096
        || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "plant observation labels must be nonempty, trimmed and bounded",
        ));
    }
    Ok(value.into())
}

/// Parsed separately from historical registration admission. An older registration's optional source
/// interpretation cannot alter its replay; only the new outcome operation requires this complete scope.
#[derive(Debug, Clone)]
pub(in crate::forecast) struct TargetScope {
    target: Target,
    boundary: String,
    circuit: String,
    blend: String,
    material_domain: String,
    dry_basis: String,
    element: String,
    start_ns: i64,
    end_ns: i64,
}

pub(in crate::forecast) fn registered_scope(row: &Row<'_>, operation: &str) -> Result<TargetScope> {
    let start_ns = registration_start(row, operation)?;
    let retained = operation
        .lines()
        .find_map(|line| {
            line.strip_prefix(super::EVIDENCE_PREFIX)
                .or_else(|| line.strip_prefix(super::LEGACY_EVIDENCE_PREFIX))
        })
        .ok_or_else(|| absent("plant registration evidence missing"))?;
    let retained = parse(retained.as_bytes()).map_err(invalid)?;
    let target = Target::parse(retained_text(&retained, "target")?)?;
    if retained_text(&retained, "unit")? != target.unit() {
        return Err(changed("registration and retained target units differ"));
    }
    let production = retained_field(retained_field(&retained, "source_receipt")?, "production")?;
    if !matches!(
        retained_text(production, "schema")?,
        "fmine.plant-production-result.v1" | "fmine.plant-production-result.v2"
    ) || retained_text(production, "receipt_digest")?
        != retained_text(&retained, "receipt_digest")?
        || row.string("model_key")?
            != format!(
                "plant-production:{}:{}",
                target.as_str(),
                retained_text(&retained, "receipt_digest")?
            )
    {
        return Err(changed(
            "registration and retained production identity differ",
        ));
    }
    let scope = retained_field(production, "scope")?;
    let integer = |key| {
        retained_text(scope, key)?
            .parse::<i64>()
            .map_err(|_| invalid("retained plant support needs signed nanoseconds"))
    };
    let result = TargetScope {
        target,
        boundary: label(retained_text(scope, "boundary")?)?,
        circuit: label(retained_text(scope, "circuit")?)?,
        blend: label(retained_text(scope, "blend")?)?,
        material_domain: label(retained_text(scope, "material_domain")?)?,
        dry_basis: label(retained_text(scope, "dry_basis")?)?,
        element: label(retained_text(scope, "element")?)?,
        start_ns: integer("start_ns")?,
        end_ns: integer("end_ns")?,
    };
    if result.start_ns != start_ns
        || result.end_ns != row.time("outcome_due")?
        || row.string("site")? != result.boundary
        || row.string("domain")? != format!("{}/{}", result.circuit, result.blend)
        || row.string("quantity")? != target.quantity(&result.element)
    {
        return Err(changed(
            "registration metadata and retained physical scope differ",
        ));
    }
    Ok(result)
}

fn target_scope(state: &Replay, id: ObjectId) -> Result<&TargetScope> {
    state
        .plant_outcome_scopes
        .get(&id)
        .ok_or_else(|| absent("outcome needs an original plant registration"))?
        .as_ref()
        .map_err(Clone::clone)
}

pub(in crate::forecast) fn validate_cohort(state: &Replay, name: &str) -> Result<()> {
    let cohort = state
        .cohorts
        .get(name)
        .and_then(|id| state.ledger.cohort(*id))
        .ok_or_else(|| absent("frozen plant cohort missing"))?;
    if cohort.spec.resolution_rule != PRODUCTION_RULE {
        return Ok(());
    }
    let mut basis = None;
    for id in &cohort.spec.members {
        let scope = target_scope(state, *id)?;
        let element = (scope.target != Target::DryTonnes).then_some(scope.element.as_str());
        let member_basis = (scope.target, element);
        if !ObservationKind::Production.accepts(scope.target)
            || basis.is_some_and(|previous| previous != member_basis)
            || state.public_bound(*id)?
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "a production cohort requires original plant forecasts of the same mass target and, for metal, the same element",
            ));
        }
        basis = Some(member_basis);
    }
    Ok(())
}

pub(in crate::forecast) fn guard_supplied_outcome(state: &Replay, row: &Row<'_>) -> Result<()> {
    let id = state.forecast_id(row.string("forecast")?)?;
    let unresolved_missing =
        row.string("kind")? == "missing" && state.ledger.outcomes(id).is_empty();
    if !unresolved_missing
        && state.cohorts.values().any(|cohort_id| {
            state.ledger.cohort(*cohort_id).is_some_and(|cohort| {
                cohort.spec.resolution_rule == PRODUCTION_RULE && cohort.spec.members.contains(&id)
            })
        })
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "the frozen plant production rule requires resolve-plant and retained materials for a numeric outcome",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservationKind {
    Recovery,
    Production,
}

impl ObservationKind {
    const fn table(self) -> &'static str {
        match self {
            Self::Recovery => "plant_outcome",
            Self::Production => "plant_production_outcome",
        }
    }

    const fn rule(self) -> &'static str {
        match self {
            Self::Recovery => RULE,
            Self::Production => PRODUCTION_RULE,
        }
    }

    const fn accepts(self, target: Target) -> bool {
        matches!(
            (self, target),
            (Self::Recovery, Target::Recovery)
                | (
                    Self::Production,
                    Target::DryTonnes | Target::ContainedMetal | Target::RecoveredMetal
                )
        )
    }
}

struct Observation {
    kind: ObservationKind,
    source: String,
    vintage: String,
    available_ns: i64,
    circuit: String,
    blend: String,
    material_domain: String,
    dry_basis: String,
    element: String,
    measurand: String,
    paths: [String; 3],
}

impl Observation {
    fn parse(text: &str) -> Result<Self> {
        let doc = parse_subset(text).map_err(|error| invalid(error.to_string()))?;
        if doc.top.len() != 1 || doc.rows.len() != 1 || doc.rows[0].table != "observation" {
            return Err(invalid(
                "plant observation needs one schema and one [[observation]] table",
            ));
        }
        let kind = match doc.top_get("schema").map(|f| &f.value) {
            Some(TomlValue::Str(value)) if value == RECOVERY_SCHEMA => ObservationKind::Recovery,
            Some(TomlValue::Str(value)) if value == PRODUCTION_SCHEMA => {
                ObservationKind::Production
            }
            _ => {
                return Err(invalid(format!(
                    "plant observation schema must be {RECOVERY_SCHEMA} or {PRODUCTION_SCHEMA}"
                )));
            }
        };
        let row = Row(&doc.rows[0]);
        row.allowed(
            &[
                "source",
                "vintage",
                "available_at",
                "circuit",
                "blend",
                "material_domain",
                "dry_basis",
                "element",
                "measurand",
                "balance_basis",
                "feed",
                "concentrate",
                "tailings",
            ],
            &[],
        )?;
        if row.string("balance_basis")? != BASIS {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "declare complete-two-product/no-inventory-change explicitly",
            ));
        }
        let element = label(row.string("element")?)?;
        let measurand = label(row.string("measurand")?)?;
        if measurand != format!("total-{element}/all-phases") {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "a total-element/all-phases measurand is required; no chemical conversion is inferred",
            ));
        }
        let dry_basis = label(row.string("dry_basis")?)?;
        if dry_basis != DRY_BASIS {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "observations require total-non-water-dry-matter/v1; no dry-basis conversion is inferred",
            ));
        }
        Ok(Self {
            kind,
            source: identifier(row.string("source")?)?.into(),
            vintage: label(row.string("vintage")?)?,
            available_ns: time(row.string("available_at")?)?,
            circuit: label(row.string("circuit")?)?,
            blend: label(row.string("blend")?)?,
            material_domain: label(row.string("material_domain")?)?,
            dry_basis,
            element,
            measurand,
            paths: [
                label(row.string("feed")?)?,
                label(row.string("concentrate")?)?,
                label(row.string("tailings")?)?,
            ],
        })
    }
}

struct Snapshot {
    text: String,
    source: SourceAnchor,
}

impl Snapshot {
    fn read(path: &Path, locator: &str) -> Result<Self> {
        let (text, source) =
            crate::worksheet_input::read(path, locator, MAX_OPERATION_BYTES as u64)?;
        Ok(Self { text, source })
    }

    fn json(&self) -> String {
        format!(
            "{{\"path\":{},\"sha256\":{},\"locator\":{},\"bytes\":{},\"submitted_worksheet\":{}}}",
            js(&self.source.artifact_path),
            js(&Digest32(self.source.artifact_sha256).to_hex()),
            js(&self.source.locator),
            self.text.len(),
            js(&self.text),
        )
    }

    fn retained(value: &Json) -> Result<Self> {
        let text = retained_text(value, "submitted_worksheet")?.to_string();
        let digest = hex(retained_text(value, "sha256")?)?;
        if text.len() > MAX_OPERATION_BYTES
            || retained_field(value, "bytes")?.u64() != Some(text.len() as u64)
            || sha256(text.as_bytes()) != digest
        {
            return Err(changed(
                "retained observation bytes, length or SHA-256 differ",
            ));
        }
        let source = SourceAnchor::new(
            label(retained_text(value, "path")?)?,
            digest.0,
            label(retained_text(value, "locator")?)?,
        );
        Ok(Self { text, source })
    }
}

fn material_paths(descriptor: &Snapshot, observation: &Observation) -> Result<[PathBuf; 3]> {
    let parent = Path::new(&descriptor.source.artifact_path)
        .parent()
        .ok_or_else(|| invalid("observation descriptor needs a parent directory"))?;
    Ok(observation.paths.each_ref().map(|path| parent.join(path)))
}

struct Candidate {
    id: ObjectId,
    cohort: Digest32,
    value: f64,
    available_ns: i64,
    citation: OutcomeCitation,
    evidence: String,
    markdown: String,
}

#[derive(Debug, Clone)]
pub(in crate::forecast) struct Admission {
    json: String,
    markdown: String,
}

impl Admission {
    pub(in crate::forecast) fn json(&self) -> &str {
        &self.json
    }

    pub(in crate::forecast) fn markdown(&self) -> &str {
        &self.markdown
    }
}

fn calculate(
    state: &Replay,
    cohort_name: &str,
    alias: &str,
    descriptor: &Snapshot,
    observation: &Observation,
    sources: &[Snapshot; 3],
) -> Result<Candidate> {
    let id = state.forecast_id(alias)?;
    let scope = target_scope(state, id)?;
    if !observation.kind.accepts(scope.target) {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            if scope.target == Target::Throughput {
                "a material balance measures output over its physical interval, and cannot establish operating-capacity throughput"
            } else {
                "observation schema does not match the originally registered target"
            },
        ));
    }
    let forecast = state
        .ledger
        .forecast(id)
        .ok_or_else(|| absent("plant forecast missing"))?;
    if state.public_bound(id)? {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "a plant observation cannot replace a frozen public outcome",
        ));
    }
    if !state.ledger.outcomes(id).is_empty() {
        return Err(fail(
            RefusalCode::FrozenRecordMutation507,
            "this plant forecast already has an outcome event; no replacement is inferred",
        ));
    }
    let cohort_id = *state
        .cohorts
        .get(cohort_name)
        .ok_or_else(|| absent("frozen plant cohort missing"))?;
    let cohort = state
        .ledger
        .cohort(cohort_id)
        .ok_or_else(|| absent("frozen plant cohort missing"))?;
    if !cohort.spec.members.contains(&id)
        || cohort.spec.frozen_at_ns >= scope.start_ns
        || cohort.spec.resolution_rule != observation.kind.rule()
        || !cohort.spec.eligible_sources.contains(&observation.source)
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "frozen cohort must include this forecast before physical start and declare the exact plant outcome rule and eligible source",
        ));
    }
    if observation.circuit != scope.circuit
        || observation.blend != scope.blend
        || observation.material_domain != scope.material_domain
        || observation.dry_basis != scope.dry_basis
        || observation.element != scope.element
        || forecast.freeze_time_ns != scope.end_ns
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "observation circuit, blend, material domain, dry basis and element must match the frozen production scope",
        ));
    }
    if observation.available_ns <= scope.end_ns {
        return Err(fail(
            RefusalCode::LateRegistration206,
            "observation availability must strictly follow the complete physical interval",
        ));
    }
    for (path, source) in material_paths(descriptor, observation)?.iter().zip(sources) {
        if source.source.artifact_path != path.to_string_lossy() {
            return Err(changed(
                "retained material path differs from the observation declaration",
            ));
        }
    }
    let receipt = two_product::evaluate(sources.each_ref().map(|s| (s.text.as_str(), &s.source)))?;
    for stream in receipt.streams() {
        let measured = &stream.summary().scope;
        if measured.boundary != scope.boundary
            || measured.physical_time.start_ns != scope.start_ns
            || measured.physical_time.end_ns != scope.end_ns
        {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "all three material observations must cover the frozen boundary and exact UTC interval",
            ));
        }
    }
    let balance = &receipt.claim().value;
    let element = balance
        .elements
        .iter()
        .find(|row| {
            row.element.element == observation.element
                && row.element.measurand == observation.measurand
        })
        .ok_or_else(|| absent("the declared total element is missing from the material balance"))?;
    if balance.dry_residual_kg != ZERO || element.residual_kg != Some(ZERO) {
        return Err(fail(
            RefusalCode::Conservation101,
            "stock-neutral dry matter and selected elemental balances must close exactly; no plug or tolerance is supplied",
        ));
    }
    let [Some(feed), Some(concentrate), Some(_)] = element.stream_metal_kg else {
        return Err(absent(
            "all three streams need the selected total-element assay",
        ));
    };
    let [feed_stream, _, _] = receipt.streams();
    let dry = feed_stream.summary().amounts.dry_solids_kg;
    let (amount, calculation) = match scope.target {
        Target::Recovery => {
            if feed.signum() <= 0 {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    "positive feed metal is required to define recovery",
                ));
            }
            let ratio = exact(concentrate.checked_div(feed))?;
            if ratio.signum() < 0 || exact(ratio.checked_cmp(ONE))?.is_gt() {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    "recovery is outside [0, 1]",
                ));
            }
            (ratio, "concentrate/feed")
        }
        Target::DryTonnes => (
            exact(dry.checked_div(ExactQty::from_integer(1000)))?,
            "feed-dry-kg/1000",
        ),
        Target::ContainedMetal => (feed, "feed-metal-kg"),
        Target::RecoveredMetal => (concentrate, "concentrate-metal-kg"),
        Target::Throughput => {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "operating-capacity throughput requires its own evidence",
            ));
        }
    };
    let value = amount.to_f64();
    if !value.is_finite() || (value == 0.0 && amount.signum() != 0) {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "the exact observation has no finite non-underflowing binary64 scoring projection",
        ));
    }
    let quantity = |q: ExactQty| {
        format!(
            "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
            q.num(),
            q.den()
        )
    };
    // Source evidence deliberately excludes model and cohort identities: comparing two forecasts of
    // the same physical observation must retain identical complete outcome citations.
    let evidence = if observation.kind == ObservationKind::Recovery {
        format!(
            "{{\"schema\":\"fmine.plant-recovery-evidence.v1\",\"method\":{},\"source_class\":\"source_derived_unverified\",\"unit\":\"fraction\",\"feed_metal_kg\":{},\"concentrate_metal_kg\":{},\"exact_value\":{},\"value\":{value},\"scalar_projection\":\"binary64_from_exact_ratio\",\"descriptor\":{},\"sources\":{{\"feed\":{},\"concentrate\":{},\"tailings\":{}}},\"balance\":{},\"no_claim\":{}}}",
            js(RULE),
            quantity(feed),
            quantity(concentrate),
            quantity(amount),
            descriptor.json(),
            sources[0].json(),
            sources[1].json(),
            sources[2].json(),
            receipt.to_json(),
            js(RECOVERY_NO_CLAIM),
        )
    } else {
        format!(
            "{{\"schema\":\"fmine.plant-production-evidence.v1\",\"method\":{},\"source_class\":\"source_derived_unverified\",\"target\":{},\"unit\":{},\"calculation\":{},\"exact_value\":{},\"value\":{value},\"scalar_projection\":\"binary64_from_exact_ratio\",\"descriptor\":{},\"sources\":{{\"feed\":{},\"concentrate\":{},\"tailings\":{}}},\"balance\":{},\"no_claim\":{}}}",
            js(PRODUCTION_RULE),
            js(scope.target.as_str()),
            js(scope.target.unit()),
            js(calculation),
            quantity(amount),
            descriptor.json(),
            sources[0].json(),
            sources[1].json(),
            sources[2].json(),
            receipt.to_json(),
            js(NO_CLAIM),
        )
    };
    let evidence_digest = sha256(evidence.as_bytes());
    let citation = OutcomeCitation {
        source: observation.source.clone(),
        vintage: observation.vintage.clone(),
        locator: format!(
            "{}#{calculation}:{}",
            descriptor.source.artifact_path, scope.element
        ),
        sha256: evidence_digest.0,
    };
    let markdown = if observation.kind == ObservationKind::Recovery {
        format!(
            "\n## Source-derived plant recovery\n\n{}\n\nExact recovery: {}/{} (fraction); binary64 scoring value: {value}. Feed metal: {}/{} kg. Concentrate metal: {}/{} kg. Dry and selected-element residuals are exactly zero.\n\nObservation evidence SHA-256: {}. Source class: source_derived_unverified.\n\n<details><summary>Retained observation sources and exact balance</summary>\n\n~~~json\n{evidence}\n~~~\n\n</details>\n",
            RECOVERY_NO_CLAIM,
            amount.num(),
            amount.den(),
            feed.num(),
            feed.den(),
            concentrate.num(),
            concentrate.den(),
            evidence_digest,
        )
    } else {
        format!(
            "\n## Source-derived plant production\n\n{NO_CLAIM}\n\nTarget `{}`. Exact amount: {}/{} ({}), computed as `{calculation}`. Binary64 scoring value: {value}. Dry and selected-element residuals are exactly zero.\n\nObservation evidence SHA-256: {evidence_digest}. Source class: source_derived_unverified.\n\n<details><summary>Retained observation sources and exact balance</summary>\n\n~~~json\n{evidence}\n~~~\n\n</details>\n",
            scope.target.as_str(),
            amount.num(),
            amount.den(),
            scope.target.unit(),
        )
    };
    Ok(Candidate {
        id,
        cohort: cohort_id,
        value,
        available_ns: observation.available_ns,
        citation,
        evidence,
        markdown,
    })
}

fn request(table: &str, alias: &str, cohort: &str, candidate: &Candidate) -> String {
    format!(
        "[[{table}]]\nforecast = {}\nforecast_id = {}\ncohort = {}\ncohort_digest = {}\nvalue_bits = \"{:016x}\"\nevidence = {}\n",
        js(alias),
        js(&candidate.id.to_string()),
        js(cohort),
        js(&candidate.cohort.to_hex()),
        candidate.value.to_bits(),
        js(&candidate.evidence),
    )
}

pub(in crate::forecast) fn prepare(
    root: &Path,
    state: &Replay,
    cohort: &str,
    alias: &str,
    input: &Path,
) -> Result<(String, &'static str)> {
    let mut descriptor = Snapshot::read(&root.join(input), "plant recovery observation")?;
    let observation = Observation::parse(&descriptor.text)?;
    let locator = if observation.kind == ObservationKind::Recovery {
        "plant recovery material"
    } else {
        descriptor.source.locator = "plant production observation".into();
        "plant production material"
    };
    let [feed, concentrate, tailings] = material_paths(&descriptor, &observation)?
        .each_ref()
        .map(|path| Snapshot::read(path, locator));
    let sources = [feed?, concentrate?, tailings?];
    let candidate = calculate(state, cohort, alias, &descriptor, &observation, &sources)?;
    let table = observation.kind.table();
    let request = request(table, alias, cohort, &candidate);
    if request.len() > MAX_OPERATION_BYTES {
        return Err(invalid(
            "complete plant outcome evidence exceeds the 64 KiB operation cap",
        ));
    }
    Ok((request, table))
}

pub(in crate::forecast) fn apply(
    state: &mut Replay,
    row: Row<'_>,
    lower: i64,
    upper: i64,
) -> Result<()> {
    row.allowed(
        &[
            "forecast",
            "forecast_id",
            "cohort",
            "cohort_digest",
            "value_bits",
            "evidence",
        ],
        &[],
    )?;
    let retained = parse(row.string("evidence")?.as_bytes()).map_err(invalid)?;
    let descriptor = Snapshot::retained(retained_field(&retained, "descriptor")?)?;
    let observation = Observation::parse(&descriptor.text)?;
    if row.0.table != observation.kind.table() {
        return Err(changed(
            "observation schema does not match the stored plant outcome operation",
        ));
    }
    let sources = retained_field(&retained, "sources")?;
    let sources = [
        Snapshot::retained(retained_field(sources, "feed")?)?,
        Snapshot::retained(retained_field(sources, "concentrate")?)?,
        Snapshot::retained(retained_field(sources, "tailings")?)?,
    ];
    let candidate = calculate(
        state,
        row.string("cohort")?,
        row.string("forecast")?,
        &descriptor,
        &observation,
        &sources,
    )?;
    if row.string("forecast_id")? != candidate.id.to_string()
        || row.string("cohort_digest")? != candidate.cohort.to_hex()
        || row.string("value_bits")? != format!("{:016x}", candidate.value.to_bits())
        || row.string("evidence")? != candidate.evidence
    {
        return Err(changed(
            "retained plant outcome binding, scalar or complete evidence differs from recomputation",
        ));
    }
    if candidate.available_ns > lower {
        return Err(fail(
            RefusalCode::LateRegistration206,
            "observation availability exceeds the writer's earliest possible clock time",
        ));
    }
    let admission = Admission {
        json: format!(
            "{{\"basis\":\"source_derived_unverified\",\"forecast_id\":{},\"cohort_digest\":{},\"source_bytes_reverified_on_replay\":true,\"source_authentication\":false,\"observation_sha256\":{},\"observation\":{}}}",
            js(&candidate.id.to_string()),
            js(&candidate.cohort.to_hex()),
            js(&Digest32(candidate.citation.sha256).to_hex()),
            candidate.evidence,
        ),
        markdown: candidate.markdown,
    };
    let seq = state.ledger.append_outcome(
        candidate.id,
        candidate.available_ns,
        upper,
        OutcomeKind::Published {
            value: candidate.value,
            citation: candidate.citation,
        },
    )?;
    state
        .plant_outcome_admissions
        .insert((candidate.id, seq), admission);
    Ok(())
}
