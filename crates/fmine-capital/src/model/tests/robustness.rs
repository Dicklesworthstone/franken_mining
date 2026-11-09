//! Source-bound opt-in stress, complete metric admission and legacy/replay dispatch controls.
//! Determinism: Exact

use super::*;

const HEADER: &str = r#"
[[robustness]]
method = "wasserstein_1"
radius = "1/4"
metric_basis = "Declared unit distance between distinct scenarios"
radius_basis = "Synthetic stress budget; no probability coverage inferred"
"#;

fn distance(from: &str, to: &str, value: &str) -> String {
    format!("\n[[transport_distance]]\nfrom = \"{from}\"\nto = \"{to}\"\nvalue = \"{value}\"\n")
}

fn geometry(states: &[&str]) -> String {
    let mut out = HEADER.to_string();
    for from in states {
        for to in states {
            out.push_str(&distance(from, to, if from == to { "0/1" } else { "1/1" }));
        }
    }
    out
}

#[test]
fn declared_source_geometry_reaches_existing_model_dispatch_with_every_anchor() {
    let text = format!("{STATIC}{}", geometry(&["base", "delay"]));
    let admitted = parse(&text, source(&text)).unwrap();
    let design = admitted.robustness.as_ref().unwrap();
    assert_eq!(design.value.radius, Rat::new(1, 4).unwrap());
    assert_eq!(design.value.distances.len(), 4);
    assert_eq!(
        design
            .anchors
            .iter()
            .filter(|anchor| anchor.locator.contains("transport distance"))
            .count(),
        4
    );
    assert!(
        design
            .anchors
            .iter()
            .all(|anchor| anchor.artifact_sha256 == sha256(text.as_bytes()).0)
    );
    let direct = crate::evaluate(&admitted).unwrap();
    let dispatched = evaluate(&text, source(&text)).unwrap();
    assert_eq!(direct.to_json(), dispatched.to_json());
    assert_eq!(direct.to_markdown(), dispatched.to_markdown());
    assert!(direct.claim().parents.contains(&design.claim_id));
    assert!(direct.to_json().contains("\"robustness\""));
    let annotated = format!("# Additional source annotation\n{text}");
    assert_eq!(
        parse(&annotated, source(&text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let changed = evaluate(&annotated, source(&annotated)).unwrap();
    assert_ne!(direct.to_json(), changed.to_json());
}

#[test]
fn strict_parser_refuses_orphans_missing_pairs_duplicates_unknowns_and_invalid_metrics() {
    let complete = geometry(&["base", "delay"]);
    let first = distance("base", "base", "0/1");
    let last = distance("delay", "delay", "0/1");
    let invalid = [
        format!("{complete}{HEADER}"),
        complete.replace(&last, ""),
        format!("{complete}{first}"),
        first,
        complete.replace("radius = \"1/4\"", "radius = \"-1/4\""),
        complete.replace("radius = \"1/4\"", "radius = \"0.25\""),
        complete.replace("radius = \"1/4\"", "radius = \"1/0\""),
        complete.replace(
            "radius = \"1/4\"",
            "radius = \"1/4\"\nconfidence = \"0.95\"",
        ),
        complete.replace("from = \"base\"", "from = \"unknown\""),
        complete.replace(
            &distance("base", "base", "0/1"),
            &distance("base", "base", "1/1"),
        ),
        complete.replace(
            &distance("base", "delay", "1/1"),
            &distance("base", "delay", "2/1"),
        ),
    ];
    for extra in invalid {
        let text = format!("{STATIC}{extra}");
        assert!(parse(&text, source(&text)).is_err(), "{extra}");
    }
}

#[test]
fn both_source_dispatch_and_direct_recourse_reject_static_stress_on_adaptive_policies() {
    let text = format!("{STAGED}{}", geometry(&["good", "bad"]));
    assert_eq!(
        evaluate(&text, source(&text)).unwrap_err().code,
        "FMINE-REF-201"
    );
    let (mut program, information) = parse_model(STAGED, source(STAGED), true).unwrap();
    let declared = parse(
        &format!("{STATIC}{}", geometry(&["base", "delay"])),
        source(&format!("{STATIC}{}", geometry(&["base", "delay"]))),
    )
    .unwrap();
    // Even a design from another model is refused as unsupported recourse before it can be applied.
    program.robustness = declared.robustness;
    let refusal = crate::recourse::evaluate(&RecourseInputs {
        program,
        information: information.unwrap(),
    })
    .unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-201");
    assert!(refusal.what.contains("static bundles only"));
}

#[test]
fn without_opt_in_the_original_static_and_staged_models_keep_no_stress_fields() {
    for text in [STATIC, STAGED] {
        let report = evaluate(text, source(text)).unwrap();
        assert!(!report.to_json().contains("\"robustness\""));
        assert!(
            !report
                .to_markdown()
                .contains("Capital scenario-weight robustness")
        );
    }
}
