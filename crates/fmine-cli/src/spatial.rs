//! DESIGN (BD-051, BD-311): read-only spatial calibration evidence from a declared mining front.
//! Determinism: DeterministicPerIsa (inherited receipt; exact source bytes and ordered rows).

use std::path::{Path, PathBuf};

use fmine_calib::spatial as monitor;
use fmine_evidence::Refusal;

use crate::{CliCommand, Envelope, NextAction, journal_arguments, worksheet_input};

pub const USAGE: &str =
    "forecast spatial-monitor --input <worksheet.toml> [--as-of-ns <UTC Unix nanoseconds>]";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.spatial-monitor.v1","usage":"forecast spatial-monitor --input <worksheet.toml> [--as-of-ns <UTC Unix nanoseconds>]","mode":"Read-only assessment of a declared ordered mining front; no journal append or independent prospective time proof","policy":["model_id","target_unit","coordinate_frame","support","nugget_var","sill_var","range_m","max_neighbors","alpha","registered_ns"],"observations":["id","available_ns","prediction_registered_ns","x_m","y_m","z_m","prior_mean","observed_value","target_unit","coordinate_frame","support","locator"],"coordinates":"Metres in one explicitly declared frame; variances are in squared target units; no implicit conversions","ordering":"Strictly increasing declared availability; the model policy and each admitted prior prediction precede the corresponding outcome","cutoff":"Optional inclusive declared availability cutoff. Binds the full input hash; only the prefix available by that instant appears in diagnostics and can trigger a reported alarm. The policy must have been declared by the cutoff","output":"Source hash, declared model and admitted observations, conditional mean and spread, standardized residual, PIT, location and dispersion evidence, and sticky first alarm","evidence":"Assumed and Internal; calibration of the model, measurement-error separation, authenticated observations, external timestamps and human case resolution are not established","example":"tests/forecasts/spatial_synthetic.toml","limits":{"bytes":4194304,"observations":4096,"nearest_predecessors":"1 through 16"},"field_types":{"integer":["registered_ns","max_neighbors","available_ns","prediction_registered_ns"],"other_fields":"Quoted strings; numerical strings must be finite"},"supports":["mining_polygon","point_core"],"time_basis":"Nonnegative UTC Unix nanoseconds; supplied declarations, not independently proven times"}"#;

pub fn parse(args: &[String]) -> Result<CliCommand, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options
        .remove("--input")
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| USAGE.to_string())?;
    let as_of_ns = options
        .remove("--as-of-ns")
        .map(|value| {
            value
                .parse::<i64>()
                .ok()
                .filter(|time| *time >= 0)
                .ok_or_else(|| "--as-of-ns requires nonnegative UTC Unix nanoseconds".to_string())
        })
        .transpose()?;
    if words != ["forecast", "spatial-monitor"] || !options.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok(CliCommand::ForecastSpatial {
        input: PathBuf::from(input),
        as_of_ns,
    })
}

pub fn execute(input: &Path, as_of_ns: Option<i64>, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "spatial monitor worksheet",
            monitor::MAX_BYTES,
        )?;
        let receipt = match as_of_ns {
            Some(cutoff) => monitor::evaluate_as_of(&text, source.clone(), cutoff)?,
            None => monitor::evaluate(&text, source.clone())?,
        };
        let payload = format!(
            "{{\"schema\":\"fmine.spatial-monitor-command.v1\",\"source\":{},\"assessment\":{}}}",
            worksheet_input::source_json(&source, text.len()),
            receipt.to_json(),
        );
        let mut envelope = Envelope::ok("forecast spatial-monitor", payload, Vec::new());
        envelope.no_claim = Some(monitor::NO_CLAIM);
        envelope.warnings.push(
            "The model, physical scope and observation times are declared assumptions; an alarm requires investigation."
                .into(),
        );
        envelope.rendered_markdown = Some(receipt.to_markdown());
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "forecast spatial-monitor",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect spatial monitoring policy, observation ordering and source requirements",
            )],
        )
    })
}
