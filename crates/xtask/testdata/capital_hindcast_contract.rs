// Binary workflow for a synthetic approval and separate closeout (BD-230).
// Determinism: Exact

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const APPROVAL: &str = include_str!("../../../tests/capital/hindcast_approval.toml");
const OUTCOME: &str = include_str!("../../../tests/capital/hindcast_outcome.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-capital-hindcast-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("approval.toml"), APPROVAL).unwrap();
        Self(root)
    }

    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args(args)
            .arg("--workspace")
            .arg(&self.0)
            .output()
            .unwrap()
    }

    fn freeze(&self) -> Json {
        envelope(
            self.invoke(&["decide", "hindcast", "freeze", "approval.toml"]),
            0,
        )
    }

    fn score(&self, digest: &str, code: i32) -> Json {
        envelope(
            self.invoke(&[
                "decide",
                "hindcast",
                "score",
                "approval.toml",
                "--outcome",
                "outcome.toml",
                "--approval-digest",
                digest,
            ]),
            code,
        )
    }

    fn approval(&self, text: &str) {
        fs::write(self.0.join("approval.toml"), text).unwrap();
    }

    fn outcome(&self, text: &str) {
        fs::write(self.0.join("outcome.toml"), text).unwrap();
    }
}

fn envelope(output: Output, code: i32) -> Json {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse(&output.stdout).unwrap()
}

fn get<'a>(json: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(json, |value, key| value.get(key).unwrap())
}

fn text<'a>(json: &'a Json, keys: &[&str]) -> &'a str {
    get(json, keys).as_str().unwrap()
}

#[test]
fn freezes_before_outcome_and_preserves_approval_on_revised_score() {
    let f = Fixture::new();
    let freeze = f.freeze();
    let p = get(&freeze, &["payload"]);
    assert_eq!(text(p, &["mode"]), "freeze");
    assert_eq!(text(p, &["approval_cut"]), "2023-02-24T15:00:00Z");
    assert_eq!(text(p, &["h7a_choice"]), "approve_full");
    assert_eq!(text(p, &["h7d_choice"]), "stage_with_recourse");
    assert_eq!(get(p, &["score"]), &Json::Null);
    let actions = get(p, &["actions"]).as_array().unwrap();
    assert_eq!(actions.len(), 3);
    assert_eq!(
        text(&actions[0], &["minimum_liquidity_slack_usd_m"]),
        "-100"
    );
    assert_eq!(
        text(&actions[1], &["expected_delta_nav_usd_per_share"]),
        "129/500"
    );
    assert_eq!(text(&actions[1], &["cvar_loss_usd_per_share"]), "3/25");
    assert_eq!(text(p, &["policy", "max_cvar_loss_usd_per_share"]), "3/10");
    assert_eq!(freeze, f.freeze());
    assert_eq!(
        text(p, &["approval_source", "sha256"]),
        sha256(APPROVAL.as_bytes()).to_hex()
    );
    let digest = text(p, &["approval_digest"]);

    f.outcome(OUTCOME);
    let result = f.score(digest, 0);
    let score = get(&result, &["payload", "score"]);
    assert_eq!(text(score, &["h7d_advantage_usd_per_share"]), "2/5");
    assert_eq!(text(score, &["capex_multiple"]), "35/18");
    assert_eq!(
        text(score, &["source", "sha256"]),
        sha256(OUTCOME.as_bytes()).to_hex()
    );
    assert_eq!(text(&result, &["payload", "approval_digest"]), digest);
    let md = f.invoke(&[
        "decide",
        "hindcast",
        "score",
        "approval.toml",
        "--approval-digest",
        digest,
        "--outcome",
        "outcome.toml",
        "--format",
        "markdown",
    ]);
    assert_eq!(md.status.code(), Some(0));
    let md = String::from_utf8(md.stdout).unwrap();
    assert!(md.contains("Supplied H7d payoff -1/10 USD/share"));
    assert!(md.contains("counterfactual"));
    assert!(md.contains(&sha256(OUTCOME.as_bytes()).to_hex()));
    assert!(
        String::from_utf8_lossy(
            &f.invoke(&["decide", "hindcast", "freeze", "approval.toml"])
                .stdout
        )
        .contains("\"execution_authorized\":false")
    );

    f.approval(&format!("# Review annotation\n{APPROVAL}"));
    let annotated = f.score(digest, 0);
    assert_eq!(text(&annotated, &["payload", "approval_digest"]), digest);
    assert_ne!(
        text(&annotated, &["payload", "approval_source", "sha256"]),
        text(p, &["approval_source", "sha256"])
    );

    f.outcome(&OUTCOME.replace("staged_nav_usd_m = \"-100\"", "staged_nav_usd_m = \"-550\""));
    let revised = f.score(digest, 0);
    assert_eq!(text(&revised, &["payload", "approval_digest"]), digest);
    assert_eq!(
        text(&revised, &["payload", "h7d_choice"]),
        "stage_with_recourse"
    );
    assert_eq!(
        text(
            &revised,
            &["payload", "score", "h7d_advantage_usd_per_share"]
        ),
        "-1/20"
    );
    assert_ne!(
        text(&revised, &["payload", "score", "outcome_digest"]),
        text(score, &["outcome_digest"])
    );
}

#[test]
fn refuses_changed_approval_before_opening_outcome_and_checks_later_evidence() {
    let f = Fixture::new();
    let freeze = f.freeze();
    let pin = text(&freeze, &["payload", "approval_digest"]);
    f.approval(&APPROVAL.replace("hurdle_irr = \"15/100\"", "hurdle_irr = \"16/100\""));
    let changed = f.score(pin, 6);
    assert_eq!(text(&changed, &["refusal", "code"]), "FMINE-REF-507");
    assert!(text(&changed, &["refusal", "what"]).contains("approval digest changed"));

    f.approval(APPROVAL);
    f.outcome(&OUTCOME.replace("north-basin-expansion", "another-expansion"));
    let wrong = f.score(pin, 3);
    assert!(text(&wrong, &["refusal", "what"]).contains("outcome case_id differs"));

    f.outcome(&OUTCOME.replace("2025-11-20T12:00:00Z", "2023-02-24T15:00:00Z"));
    let early = f.score(pin, 3);
    assert_eq!(text(&early, &["refusal", "code"]), "FMINE-REF-108");

    f.approval(&APPROVAL.replace("2023-02-24T10:00:00Z", "2023-02-25T00:00:00Z"));
    let late = envelope(
        f.invoke(&["decide", "hindcast", "freeze", "approval.toml"]),
        3,
    );
    assert_eq!(text(&late, &["refusal", "code"]), "FMINE-REF-108");
}

#[test]
fn usage_and_robot_docs_name_the_required_pin() {
    let f = Fixture::new();
    let usage = envelope(
        f.invoke(&[
            "decide",
            "hindcast",
            "score",
            "approval.toml",
            "--outcome",
            "outcome.toml",
        ]),
        2,
    );
    assert_eq!(text(&usage, &["verb"]), "decide hindcast");
    assert!(text(&usage, &["payload", "error"]).contains("--approval-digest"));
    let docs = envelope(f.invoke(&["robot-docs"]), 0);
    assert!(text(&docs, &["payload", "capital_hindcast", "score"]).contains("--approval-digest"));
}
