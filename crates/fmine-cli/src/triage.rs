//! Bare `fmine` (`BD-114`, plan §30.2): triage of what in this workspace needs attention now.
//!
//! DESIGN: triage reads only what `fmine doctor` reads and points to it for detail. Of the plan's five triage
//! items, the one a workspace shows today is the refusal `fmine assure public --issuer <name>` would give: a
//! registered source that is missing or fails its length or digest check, or an issuer registry that does not
//! load. `not_covered` names the five items' gaps, so an empty `attention` list never reads as a clean bill for
//! something triage cannot see.

use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::RegistryCatalog;

use crate::doctor::{SourceChecks, diagnose_forecasts, issuer_source_checks};
use crate::{DOCTOR_WHY, Envelope, NextAction, ROBOT_DOCS_WHY, escape_json};

/// What each plan §30.2 triage item lacks in a workspace today.
const NOT_COVERED: [(&str, &str); 5] = [
    (
        "live_obligations",
        "the durable obligation registry and fmined are not deployed",
    ),
    ("stale_cells", "fmined is not deployed"),
    (
        "open_refusals",
        "no refusal journal is kept; attention lists the issuer runs that would refuse",
    ),
    ("gates_near_threshold", "no registry holds gate thresholds"),
    (
        "budgets_near_limits",
        "runtime budgets need fmined; fmine doctor and xtask check-loc report code-size budgets",
    ),
];

/// Machine-readable documentation block for `fmine robot-docs`.
pub(crate) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"command\":\"fmine [--json | --format json|markdown] [--workspace <dir>]\",\"verb\":\"triage\",",
    "\"schema\":\"fmine.triage.v1\",\"fields\":[\"attention\",\"issuers\",\"forecasts\",\"not_covered\"],",
    "\"scope\":\"Read-only; attention has one row per issuer whose assure public run would refuse; ",
    "not_covered names the plan's triage items this workspace cannot show yet\"}"
);

/// One issuer's source state: `(registered, present, verified)`, where verified means length and digest both
/// match the registry.
fn counts(files: &[(String, [bool; 3])]) -> (usize, usize, usize) {
    let present = files.iter().filter(|(_, [p, _, _])| *p).count();
    let verified = files.iter().filter(|(_, c)| c.iter().all(|ok| *ok)).count();
    (files.len(), present, verified)
}

/// Why `fmine assure public --issuer <name>` would refuse today, or `None` when every registered source verifies.
fn refusal_ahead(checks: &SourceChecks) -> Option<String> {
    match checks {
        Err(why) => Some(format!("its source registry does not load: {why}")),
        Ok(files) => {
            let (registered, present, verified) = counts(files);
            (verified < registered).then(|| {
                format!(
                    "{} of {registered} registered sources missing, {} failing their length or digest check",
                    registered - present,
                    present - verified
                )
            })
        }
    }
}

/// Execute bare `fmine` against `workspace_root`.
pub(crate) fn execute(workspace_root: &Path) -> Envelope {
    if let Err(err) = RegistryCatalog::load_from_root(workspace_root) {
        return Envelope::refused(
            "triage",
            Refusal::new(RefusalCode::InputMissingOrUnreadable506, err.to_string()),
            vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
        );
    }
    let issuers = issuer_source_checks(workspace_root);
    let mut attention = Vec::new();
    let mut markdown = String::from("# fmine triage\n\n");
    let mut rows = Vec::new();
    for (name, checks) in &issuers {
        let issuer = escape_json(name);
        rows.push(match checks {
            Ok(files) => {
                let (registered, present, verified) = counts(files);
                format!(
                    "{{\"issuer\":\"{issuer}\",\"registry_ok\":true,\"sources_registered\":{registered},\"sources_present\":{present},\"sources_verified\":{verified}}}"
                )
            }
            Err(_) => format!("{{\"issuer\":\"{issuer}\",\"registry_ok\":false}}"),
        });
        if let Some(what) = refusal_ahead(checks) {
            attention.push(format!(
                "{{\"area\":\"issuer_sources\",\"issuer\":\"{issuer}\",\"what\":\"{}\",\"refuses\":\"fmine assure public --issuer {issuer}\"}}",
                escape_json(&what)
            ));
            markdown.push_str(&format!(
                "- {name}: {what}. `fmine assure public --issuer {name}` would refuse.\n"
            ));
        }
    }
    if attention.is_empty() {
        markdown.push_str("- Nothing triage can see needs attention.\n");
    }
    markdown.push_str("\nNot covered:\n");
    for (item, why) in NOT_COVERED {
        markdown.push_str(&format!("- {}: {why}.\n", item.replace('_', " ")));
    }
    markdown.push_str("\nDetail: `fmine doctor`.");
    let not_covered: Vec<String> = NOT_COVERED
        .iter()
        .map(|(item, why)| format!("\"{item}\":\"{}\"", escape_json(why)))
        .collect();
    let payload = format!(
        "{{\"schema\":\"fmine.triage.v1\",\"attention\":[{}],\"issuers\":[{}],\"forecasts\":{},\"not_covered\":{{{}}}}}",
        attention.join(","),
        rows.join(","),
        diagnose_forecasts(workspace_root),
        not_covered.join(",")
    );
    let mut envelope = Envelope::ok(
        "triage",
        payload,
        vec![
            NextAction::new("fmine doctor", DOCTOR_WHY),
            NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY),
        ],
    );
    envelope.rendered_markdown = Some(markdown);
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(states: &[[bool; 3]]) -> SourceChecks {
        Ok(states
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("f{i}.xlsx"), *s))
            .collect())
    }

    #[test]
    fn test_triage_names_each_issuer_run_that_would_refuse() {
        assert_eq!(refusal_ahead(&files(&[[true; 3], [true; 3]])), None);
        assert_eq!(refusal_ahead(&files(&[])), None);
        assert_eq!(
            refusal_ahead(&files(&[[true; 3], [false; 3], [true, true, false]])).as_deref(),
            Some("1 of 3 registered sources missing, 1 failing their length or digest check")
        );
        // DESIGN: a digest that matches at the wrong length still fails; the registry pins both.
        assert_eq!(
            refusal_ahead(&files(&[[true, false, true]])).as_deref(),
            Some("0 of 1 registered sources missing, 1 failing their length or digest check")
        );
        assert_eq!(
            refusal_ahead(&Err("line 3: unknown key".into())).as_deref(),
            Some("its source registry does not load: line 3: unknown key")
        );
    }

    #[test]
    fn test_triage_of_the_repository_lists_every_issuer_and_each_refusal_ahead() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let env = execute(&root);
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "triage");
        assert!(
            env.payload_json
                .starts_with("{\"schema\":\"fmine.triage.v1\",\"attention\":[")
        );
        // The corpus is not in git, so whether Barrick's sources verify depends on the checkout; the payload
        // must agree with the checks either way.
        let issuers = issuer_source_checks(&root);
        assert!(
            issuers.iter().any(|(name, _)| name == "barrick"),
            "{issuers:?}"
        );
        for (name, checks) in &issuers {
            let row = format!("\"area\":\"issuer_sources\",\"issuer\":\"{name}\"");
            assert_eq!(
                env.payload_json.contains(&row),
                refusal_ahead(checks).is_some(),
                "{name}: {}",
                env.payload_json
            );
        }
        for (item, _) in NOT_COVERED {
            assert!(
                env.payload_json.contains(&format!("\"{item}\":\"")),
                "{item}"
            );
        }
        let docs = crate::execute_command(&crate::CliCommand::RobotDocs, &root).payload_json;
        assert!(
            docs.contains(&format!(",\"triage\":{ROBOT_DOCS_JSON}")),
            "{docs}"
        );
        let md = env.rendered_markdown.as_deref().unwrap_or_default();
        assert!(md.starts_with("# fmine triage\n\n- "), "{md}");
        assert!(md.ends_with("\nDetail: `fmine doctor`."), "{md}");
    }

    #[test]
    fn test_triage_refuses_a_root_whose_registries_do_not_load() {
        let missing =
            std::env::temp_dir().join(format!("fmine-triage-missing-{}", std::process::id()));
        let env = execute(&missing);
        assert_eq!((env.status, env.exit_code), ("refused", 5));
        assert_eq!(
            env.refusal.map(|r| r.code),
            Some(RefusalCode::InputMissingOrUnreadable506.as_str())
        );
    }
}
