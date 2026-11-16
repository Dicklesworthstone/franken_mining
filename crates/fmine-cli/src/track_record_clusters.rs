//! DESIGN (BD-341.S4): explicit independent-cluster assumption, separate from legacy track records.

use std::path::Path;

use fmine_public::guidance::track_record::{self, clusters};
use fmine_public::sources::{is_issuer_name, registry_path};

use crate::{DOCTOR_WHY, Envelope, NextAction};

pub const ROBOT_DOCS_JSON: &str = r#"{"command":"track-record clusters --issuer <issuer> --as-of YYYY-MM-DD --axis year|site [--site <site>]","synthetic":"track-record clusters --synthetic --axis year|site","schema":"fmine.guidance.track-record.clusters.v1","population":"Earliest registered guidance per site/year/metric/basis; ambiguous or missing members retained; first-published and latest-registered outcomes assessed separately","partitions":"Exact year and site cluster membership, resolved denominators, rounding bounds and missing-outcome bounds","inference":"1999 keyed Philox block resamples; inverse-ECDF nominal 90 percent interval for the equally weighted mean of the selected cluster means; minimum 20 complete varying clusters, no member deletion to meet that floor","assumption":"Independent exchangeable selected clusters are assumed, not verified; year clustering does not cover persistent site shocks, site clustering does not cover shared year shocks, and neither is a two-way robust interval","limits":"No calibrated label, effective sample size, ranking, causal attribution or historical-registration proof; sparse, incomplete and degenerate populations retain descriptive results without a sampling interval","identity":"GTCL/v1 binds the complete unchanged dated source report, source links, both outcome selectors, partition, resampling rule and results"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    issuer: Option<String>,
    as_of: Option<String>,
    site: Option<String>,
    axis: clusters::Axis,
}

pub(crate) fn parse(words: &[String], root: &Path) -> Result<Command, String> {
    let (mut fields, positional) = crate::journal_arguments(words, &["--synthetic"])?;
    if positional != ["track-record", "clusters"] {
        return Err("expected track-record clusters".into());
    }
    let axis = match fields.remove("--axis").as_deref() {
        Some("year") => clusters::Axis::Year,
        Some("site") => clusters::Axis::Site,
        _ => {
            return Err(
                "--axis requires an explicit year or site independent-cluster assumption".into(),
            );
        }
    };
    let issuer = fields.remove("--issuer");
    let as_of = fields.remove("--as-of");
    let site = fields.remove("--site");
    let synthetic = fields.remove("--synthetic").is_some();
    if !fields.is_empty() {
        return Err(format!(
            "unknown track-record clusters options: {:?}",
            fields.keys().collect::<Vec<_>>()
        ));
    }
    if synthetic {
        if issuer.is_some() || as_of.is_some() || site.is_some() {
            return Err("synthetic clusters uses its declared invented evidence cut; no issuer, as-of or site selection".into());
        }
    } else {
        let name = issuer
            .as_deref()
            .ok_or("clusters requires --issuer or --synthetic")?;
        if !is_issuer_name(name) || !registry_path(root, name).is_file() {
            return Err(format!("unknown issuer '{name}'"));
        }
        if !as_of
            .as_deref()
            .is_some_and(fmine_registry::sources::is_date)
        {
            return Err("clusters requires --as-of YYYY-MM-DD".into());
        }
        if site
            .as_deref()
            .is_some_and(|s| s.trim().is_empty() || s.chars().any(char::is_control))
        {
            return Err("--site requires a nonempty subject key".into());
        }
    }
    Ok(Command {
        issuer,
        as_of,
        site,
        axis,
    })
}

pub(crate) fn execute(command: &Command, root: &Path) -> Envelope {
    let source = match (&command.issuer, &command.as_of) {
        (Some(issuer), Some(as_of)) => {
            track_record::run(root, issuer, command.site.as_deref(), as_of)
        }
        _ => track_record::synthetic(),
    };
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match source.and_then(|report| clusters::assess(report, command.axis)) {
        Ok(report) => {
            let mut envelope = Envelope::ok("track-record clusters", report.to_json(), actions);
            envelope.no_claim = Some(clusters::NO_CLAIM);
            envelope.rendered_markdown = Some(report.to_markdown());
            envelope
        }
        Err(refusal) => Envelope::refused("track-record clusters", refusal, actions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_axis_is_required_and_synthetic_projection_keeps_all_missing_members() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let args = |words: &[&str]| words.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for words in [
            vec!["track-record", "clusters", "--synthetic"],
            vec!["track-record", "clusters", "--synthetic", "--axis", "both"],
            vec![
                "track-record",
                "clusters",
                "--synthetic",
                "--axis",
                "year",
                "--axis",
                "site",
            ],
            vec![
                "track-record",
                "clusters",
                "--synthetic",
                "--axis",
                "year",
                "--as-of",
                "2032-02-01",
            ],
            vec![
                "track-record",
                "clusters",
                "--synthetic",
                "--axis",
                "year",
                "--through",
                "2031Q4",
            ],
        ] {
            assert_eq!(
                crate::invoke(&args(&words), &root).0.exit_code,
                2,
                "{words:?}"
            );
        }
        let words = args(&[
            "--json",
            "track-record",
            "--axis",
            "year",
            "clusters",
            "--synthetic",
        ]);
        let (envelope, _) = crate::invoke(&words, &root);
        assert_eq!(envelope.exit_code, 0, "{}", envelope.payload_json);
        assert!(
            envelope
                .payload_json
                .contains("\"schema\":\"fmine.guidance.track-record.clusters.v1\"")
        );
        assert!(
            envelope
                .payload_json
                .contains("\"n_members\":2,\"n_unresolved\":1")
        );
        assert!(
            envelope
                .payload_json
                .contains("\"inference_unavailable\":\"unresolved_members\"")
        );
        assert!(envelope.payload_json.contains("\"source_report\":"));
        assert!(
            envelope
                .rendered_markdown
                .as_ref()
                .unwrap()
                .contains("Equal-cluster mean")
        );
        assert_eq!(envelope, crate::invoke(&words, &root).0);
    }
}
