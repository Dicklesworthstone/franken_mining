//! DESIGN (BD-181/263): derive a recovery outcome from complete stock-neutral material observations.
//! The exact measurement owner supplies concentrate/feed metal; no prediction or assay-implied split
//! becomes an observation. Every original worksheet remains in the bounded journal operation.
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
const SCHEMA: &str = "fmine.plant-recovery-observation.v1";
const BASIS: &str = "complete-two-product/no-inventory-change";
const DRY_BASIS: &str = "total-non-water-dry-matter/v1";
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);

pub(in crate::forecast) const NO_CLAIM: &str = "Recovery is the exact concentrate/feed total-element mass ratio from declared whole-material observations. Complete feed, concentrate and tailings, unchanged circuit inventory and exact dry/elemental closure are required. Source bytes and arithmetic are checked, but measurement accuracy, stream mapping, availability and completeness remain supplied and unverified. No assay-implied, reconciled or modelled value is substituted. The exact ratio and its binary64 scoring projection are retained; no calibration, P754 compliance, external time proof or operating authority is established.";

pub(in crate::forecast) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal resolve-plant --journal <path> --head <hex> --cohort <name> --forecast <alias> --input <observation.toml> --trust-host-clock","target":"A previously registered plant recovery forecast, weighted v14 or legacy quantile v9; other targets refuse","cohort":"Membership must be frozen before physical start, with resolution_rule plant-two-product-recovery/v1 and the observation source among its eligible_sources","schema":"fmine.plant-recovery-observation.v1","observation_fields":["source","vintage","available_at","circuit","blend","material_domain","dry_basis","element","measurand","balance_basis","feed","concentrate","tailings"],"layout":"One schema field and one [[observation]] table; values are quoted. Material paths are relative to the observation descriptor directory unless absolute","basis":"balance_basis = complete-two-product/no-inventory-change; dry_basis = total-non-water-dry-matter/v1; measurand = total-<element>/all-phases","sources":"Three existing fmine.plant-material.v1 whole-material worksheets with distinct material identities; boundary and positive UTC interval must equal the frozen forecast support, and declared circuit/blend/material-domain/dry-basis/element must match","calculation":"Exact concentrate/feed elemental kg, with positive feed metal and exact zero dry and selected-element residuals; no assay-implied split, fitted value, tolerance or clamp","time":"Declared availability must strictly follow physical end and not exceed the writer clock's earliest possible time","journal":"Opt-in operation v16 retains the descriptor, three source worksheets, source SHA-256 values, exact ratio, scalar bits and complete diagnostic receipt under the unchanged 64 KiB operation cap; older operation bytes retain their semantics","replay":"Recompute only the retained material observations, never current source files or the prediction model. Existing outcomes cannot be overwritten; missing evidence refuses atomically","interpretation":"source_derived_unverified; supplied observations and support declarations are not independently authenticated measurements or a calibration claim"}"#;

type Result<T> = std::result::Result<T, Refusal>;

fn fail(code: RefusalCode, what: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant recovery outcome: {}", what.into()))
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
    if retained_text(&retained, "target")? != Target::Recovery.as_str()
        || retained_text(&retained, "unit")? != "fraction"
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "only an originally registered plant recovery target is supported",
        ));
    }
    let production = retained_field(retained_field(&retained, "source_receipt")?, "production")?;
    if !matches!(
        retained_text(production, "schema")?,
        "fmine.plant-production-result.v1" | "fmine.plant-production-result.v2"
    ) || retained_text(production, "receipt_digest")?
        != retained_text(&retained, "receipt_digest")?
        || row.string("model_key")?
            != format!(
                "plant-production:recovery:{}",
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
        || row.string("quantity")? != Target::Recovery.quantity(&result.element)
    {
        return Err(changed(
            "registration metadata and retained physical scope differ",
        ));
    }
    Ok(result)
}

struct Observation {
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
        if doc.top.len() != 1
            || !matches!(doc.top_get("schema").map(|f| &f.value), Some(TomlValue::Str(value)) if value == SCHEMA)
            || doc.rows.len() != 1
            || doc.rows[0].table != "observation"
        {
            return Err(invalid(format!(
                "plant observation needs schema = {SCHEMA} and one [[observation]] table"
            )));
        }
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
    sources: &[Snapshot; 3],
) -> Result<Candidate> {
    let id = state.forecast_id(alias)?;
    let scope = state
        .plant_outcome_scopes
        .get(&id)
        .ok_or_else(|| absent("outcome needs an original plant registration"))?
        .as_ref()
        .map_err(Clone::clone)?;
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
            "this recovery forecast already has an outcome event; no replacement is inferred",
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
    let observation = Observation::parse(&descriptor.text)?;
    if !cohort.spec.members.contains(&id)
        || cohort.spec.frozen_at_ns >= scope.start_ns
        || cohort.spec.resolution_rule != RULE
        || !cohort.spec.eligible_sources.contains(&observation.source)
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "frozen cohort must include this forecast before physical start and declare the exact recovery rule and eligible source",
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
    for (path, source) in material_paths(descriptor, &observation)?
        .iter()
        .zip(sources)
    {
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
    let value = ratio.to_f64();
    if !value.is_finite() || (value == 0.0 && ratio.signum() != 0) {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "the exact recovery has no finite non-underflowing binary64 scoring projection",
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
    let evidence = format!(
        "{{\"schema\":\"fmine.plant-recovery-evidence.v1\",\"method\":{},\"source_class\":\"source_derived_unverified\",\"unit\":\"fraction\",\"feed_metal_kg\":{},\"concentrate_metal_kg\":{},\"exact_value\":{},\"value\":{value},\"scalar_projection\":\"binary64_from_exact_ratio\",\"descriptor\":{},\"sources\":{{\"feed\":{},\"concentrate\":{},\"tailings\":{}}},\"balance\":{},\"no_claim\":{}}}",
        js(RULE),
        quantity(feed),
        quantity(concentrate),
        quantity(ratio),
        descriptor.json(),
        sources[0].json(),
        sources[1].json(),
        sources[2].json(),
        receipt.to_json(),
        js(NO_CLAIM),
    );
    let evidence_digest = sha256(evidence.as_bytes());
    let citation = OutcomeCitation {
        source: observation.source,
        vintage: observation.vintage,
        locator: format!(
            "{}#concentrate/feed:{}",
            descriptor.source.artifact_path, scope.element
        ),
        sha256: evidence_digest.0,
    };
    let markdown = format!(
        "\n## Source-derived plant recovery\n\n{}\n\nExact recovery: {}/{} (fraction); binary64 scoring value: {value}. Feed metal: {}/{} kg. Concentrate metal: {}/{} kg. Dry and selected-element residuals are exactly zero.\n\nObservation evidence SHA-256: {}. Source class: source_derived_unverified.\n\n<details><summary>Retained observation sources and exact balance</summary>\n\n~~~json\n{evidence}\n~~~\n\n</details>\n",
        NO_CLAIM,
        ratio.num(),
        ratio.den(),
        feed.num(),
        feed.den(),
        concentrate.num(),
        concentrate.den(),
        evidence_digest,
    );
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

fn request(alias: &str, cohort: &str, candidate: &Candidate) -> String {
    format!(
        "[[plant_outcome]]\nforecast = {}\nforecast_id = {}\ncohort = {}\ncohort_digest = {}\nvalue_bits = \"{:016x}\"\nevidence = {}\n",
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
) -> Result<String> {
    let descriptor = Snapshot::read(&root.join(input), "plant recovery observation")?;
    let observation = Observation::parse(&descriptor.text)?;
    let [feed, concentrate, tailings] = material_paths(&descriptor, &observation)?
        .each_ref()
        .map(|path| Snapshot::read(path, "plant recovery material"));
    let sources = [feed?, concentrate?, tailings?];
    let candidate = calculate(state, cohort, alias, &descriptor, &sources)?;
    let request = request(alias, cohort, &candidate);
    if request.len() > MAX_OPERATION_BYTES {
        return Err(invalid(
            "complete plant outcome evidence exceeds the 64 KiB operation cap",
        ));
    }
    Ok(request)
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
        &sources,
    )?;
    if row.string("forecast_id")? != candidate.id.to_string()
        || row.string("cohort_digest")? != candidate.cohort.to_hex()
        || row.string("value_bits")? != format!("{:016x}", candidate.value.to_bits())
        || row.string("evidence")? != candidate.evidence
    {
        return Err(changed(
            "retained recovery binding, scalar or complete evidence differs from recomputation",
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
