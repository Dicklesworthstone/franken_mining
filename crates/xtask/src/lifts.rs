//! `check-lifts`: the lift registry and the file headers state the same true provenance (§27.1, L-7, bd-5vex).
//!
//! `registries/lifts.toml` has a `[[lift]]` row for each file that really derives from the named upstream file,
//! and an `[[analogue]]` row for each file written fresh that has an upstream counterpart worth watching
//! (GATE-FSIM-DRIFT). The check fails on:
//! - (a) a row whose target is missing;
//! - (b) a lifted file whose first line is not its row's §27.1 header, with all four fields: path and commit,
//!   `licence <id>`, a non-empty `Changes:` and a non-empty `Left out:`;
//! - (c) a `guard_test` that is not a `fn` in the target's test module;
//! - (d) a `commit` or `sha256_at_lift` of the wrong form, or a licence other than [`UPSTREAM_LICENCE`];
//! - (f) a file carrying a provenance line without a `[[lift]]` row, or an `[[analogue]]` target that does not
//!   open with [`ORIGINAL_HEADER`].
//!
//! (e) `check-lifts --upstream <path>` is local only, because CI has no frankensim checkout. It recomputes each
//! `sha256_at_lift` from `git show <commit>:<source>` and measures how much of the target is shared with the
//! source, flagging a file under [`MIN_OVERLAP_PERCENT`] as "not a lift". The overlap is a crude local flag: the
//! definitive evidence is the header plus a reviewer's diff.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use fmine_registry::toml_subset::{TomlRow, TomlValue, parse_subset};

pub const REGISTRY_PATH: &str = "registries/lifts.toml";
/// frankensim's `LICENSE` at the pinned commit is MIT with the OpenAI/Anthropic rider. This is asupersync's
/// SPDX-style id for the same text. Whether the rider is acceptable is RISK-17, not this check.
pub const UPSTREAM_LICENCE: &str = "LicenseRef-MIT-OpenAI-Anthropic-Rider";
/// The first line of a file written fresh, up to the upstream path.
pub const ORIGINAL_HEADER: &str =
    "//! Original FrankenMining code (not lifted). Upstream analogue: frankensim ";
/// DESIGN threshold: the real lifts measure 21–47%, the files that were never lifted 0–5%.
pub const MIN_OVERLAP_PERCENT: usize = 20;

const PROVENANCE: &str = "//! Provenance: frankensim ";
const LIFT_FIELDS: [&str; 6] = [
    "source",
    "commit",
    "sha256_at_lift",
    "licence",
    "target",
    "guard_test",
];
const ANALOGUE_FIELDS: [&str; 3] = ["target", "upstream", "note"];

/// A file that derives from `source` at `commit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lift {
    pub source: String,
    pub commit: String,
    pub sha256_at_lift: String,
    pub licence: String,
    pub target: String,
    pub guard_test: String,
}

/// A file written fresh, with the upstream file it is the counterpart of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analogue {
    pub target: String,
    pub upstream: String,
    pub note: String,
}

fn field(row: &TomlRow, key: &str) -> Result<String, String> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Str(s)) if !s.is_empty() => Ok(s.clone()),
        _ => Err(format!(
            "{REGISTRY_PATH} line {}: [[{}]] lacks {key}",
            row.line, row.table
        )),
    }
}

/// Parses the registry, refusing the first row outside the schema.
pub fn parse_lifts(text: &str) -> Result<(Vec<Lift>, Vec<Analogue>), String> {
    let doc = parse_subset(text).map_err(|e| format!("{REGISTRY_PATH}: {e}"))?;
    let schema = doc.top_get("schema_version").map(|f| &f.value);
    if doc.top.len() != 1 || schema != Some(&TomlValue::Int(2)) {
        return Err(format!(
            "{REGISTRY_PATH}: the only top-level key must be schema_version = 2"
        ));
    }
    let (mut lifts, mut analogues) = (Vec::new(), Vec::new());
    for row in &doc.rows {
        let known: &[&str] = match row.table.as_str() {
            "lift" => &LIFT_FIELDS,
            "analogue" => &ANALOGUE_FIELDS,
            other => {
                return Err(format!(
                    "{REGISTRY_PATH} line {}: unknown table [[{other}]]",
                    row.line
                ));
            }
        };
        if let Some(extra) = row.fields.iter().find(|f| !known.contains(&f.key.as_str())) {
            return Err(format!(
                "{REGISTRY_PATH} line {}: unknown field {}",
                extra.line, extra.key
            ));
        }
        if row.table == "lift" {
            lifts.push(Lift {
                source: field(row, "source")?,
                commit: field(row, "commit")?,
                sha256_at_lift: field(row, "sha256_at_lift")?,
                licence: field(row, "licence")?,
                target: field(row, "target")?,
                guard_test: field(row, "guard_test")?,
            });
        } else {
            analogues.push(Analogue {
                target: field(row, "target")?,
                upstream: field(row, "upstream")?,
                note: field(row, "note")?,
            });
        }
    }
    Ok((lifts, analogues))
}

fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Rule (b): why `first_line` is not `row`'s header, if it is not.
fn header_error(row: &Lift, first_line: &str) -> Option<String> {
    let prefix = format!("{PROVENANCE}{} @ {};", row.source, row.commit);
    let Some(rest) = first_line.strip_prefix(&prefix) else {
        return Some(format!(
            "first line does not start with `{prefix}`: `{first_line}`"
        ));
    };
    let fields = rest
        .strip_prefix(" licence ")
        .and_then(|r| r.split_once("; Changes: "))
        .and_then(|(licence, r)| {
            r.split_once("; Left out: ")
                .map(|(changes, left_out)| (licence, changes, left_out))
        });
    match fields {
        None => Some("header lacks `licence <id>; Changes: ...; Left out: ...`".to_string()),
        Some((licence, _, _)) if licence != row.licence => Some(format!(
            "header licence {licence} is not the registry's {}",
            row.licence
        )),
        Some((_, changes, left_out)) if changes.trim().is_empty() || left_out.trim().is_empty() => {
            Some("header has an empty `Changes:` or `Left out:`".to_string())
        }
        Some(_) => None,
    }
}

/// Rule (c): `guard` names a `fn` after the file's `#[cfg(test)]`.
fn has_guard_test(content: &str, guard: &str) -> bool {
    let name = guard.rsplit("::").next().unwrap_or(guard);
    content
        .split_once("#[cfg(test)]")
        .is_some_and(|(_, tests)| tests.contains(&format!("fn {name}(")))
}

fn lift_errors(row: &Lift, read: &dyn Fn(&str) -> Option<String>, errors: &mut Vec<String>) {
    let target = &row.target;
    if !is_lower_hex(&row.commit, 40) {
        errors.push(format!(
            "{target}: commit {} is not 40 hex digits",
            row.commit
        ));
    }
    if !is_lower_hex(&row.sha256_at_lift, 64) {
        errors.push(format!(
            "{target}: sha256_at_lift {} is not 64 hex digits",
            row.sha256_at_lift
        ));
    }
    if row.licence != UPSTREAM_LICENCE {
        errors.push(format!(
            "{target}: licence {} is not upstream's {UPSTREAM_LICENCE}",
            row.licence
        ));
    }
    let Some(content) = read(target) else {
        errors.push(format!("{target}: lift target is missing"));
        return;
    };
    let first_line = content.lines().next().unwrap_or("");
    if let Some(e) = header_error(row, first_line) {
        errors.push(format!("{target}: {e}"));
    }
    if !has_guard_test(&content, &row.guard_test) {
        errors.push(format!(
            "{target}: guard test {} is not in its test module",
            row.guard_test
        ));
    }
}

/// Every rule that needs no upstream checkout. `read` gives a target's text, and `provenance_files` names each
/// file that carries a provenance line.
pub fn check_rows(
    lifts: &[Lift],
    analogues: &[Analogue],
    read: &dyn Fn(&str) -> Option<String>,
    provenance_files: &[String],
) -> Vec<String> {
    let mut errors = Vec::new();
    for row in lifts {
        lift_errors(row, read, &mut errors);
    }
    for row in analogues {
        let want = format!("{ORIGINAL_HEADER}{} @ ", row.upstream);
        match read(&row.target) {
            None => errors.push(format!("{}: analogue target is missing", row.target)),
            Some(text) if !text.starts_with(&want) => errors.push(format!(
                "{}: an analogue must open with `{want}`",
                row.target
            )),
            Some(_) => {}
        }
    }
    let rows: BTreeSet<&str> = lifts.iter().map(|r| r.target.as_str()).collect();
    for file in provenance_files {
        if !rows.contains(file.as_str()) {
            errors.push(format!("{file}: provenance line without a [[lift]] row"));
        }
    }
    errors
}

fn has_provenance_line(text: &str) -> bool {
    text.lines().any(|l| l.trim_start().starts_with(PROVENANCE))
}

fn load(root: &Path) -> Result<(Vec<Lift>, Vec<Analogue>), Vec<String>> {
    let path = root.join(REGISTRY_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|e| vec![format!("failed to read {}: {e}", path.display())])?;
    parse_lifts(&text).map_err(|e| vec![e])
}

/// Checks `registries/lifts.toml` against the files it names and every provenance line in `crates/`.
pub fn check_lifts(root: &Path) -> Result<(), Vec<String>> {
    let (lifts, analogues) = load(root)?;
    let read = |rel: &str| fs::read_to_string(root.join(rel)).ok();
    let provenance_files: Vec<String> = crate::collect_rs_files(&root.join("crates"))
        .iter()
        .filter(|p| fs::read_to_string(p).is_ok_and(|t| has_provenance_line(&t)))
        .filter_map(|p| p.strip_prefix(root).ok())
        .map(|p| p.display().to_string())
        .collect();
    let errors = check_rows(&lifts, &analogues, &read, &provenance_files);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Distinct, trimmed lines over 25 characters: `(shared with source, all in target)`.
#[must_use]
pub fn overlap(target: &str, source: &str) -> (usize, usize) {
    let lines = |t: &str| -> BTreeSet<String> {
        t.lines()
            .map(str::trim)
            .filter(|l| l.len() > 25)
            .map(str::to_string)
            .collect()
    };
    let (t, s) = (lines(target), lines(source));
    (t.intersection(&s).count(), t.len())
}

/// Rule (e), local only: each lift against `upstream`, a frankensim checkout. Every row gets one JSON log line;
/// a digest mismatch or a file under [`MIN_OVERLAP_PERCENT`] puts them all on the error side.
pub fn check_upstream(root: &Path, upstream: &Path) -> Result<Vec<String>, Vec<String>> {
    let (lifts, _) = load(root)?;
    let mut failed = false;
    let mut log = Vec::new();
    for row in &lifts {
        let shown = Command::new("git")
            .arg("-C")
            .arg(upstream)
            .args(["show", &format!("{}:{}", row.commit, row.source)])
            .output()
            .map_err(|e| vec![format!("git show failed to start: {e}")])?;
        if !shown.status.success() {
            return Err(vec![format!(
                "{}: git show {}:{} failed: {}",
                row.target,
                row.commit,
                row.source,
                String::from_utf8_lossy(&shown.stderr).trim()
            )]);
        }
        let sha256 = fmine_core::hash::sha256(&shown.stdout).to_hex();
        let target = fs::read_to_string(root.join(&row.target))
            .map_err(|e| vec![format!("{}: {e}", row.target)])?;
        let (shared, lines) = overlap(&target, &String::from_utf8_lossy(&shown.stdout));
        let percent = shared * 100 / lines.max(1);
        let sha_ok = sha256 == row.sha256_at_lift;
        let not_a_lift = percent < MIN_OVERLAP_PERCENT;
        failed |= !sha_ok || not_a_lift;
        log.push(format!(
            "{{\"check\":\"check-lifts-upstream\",\"target\":\"{}\",\"source\":\"{}\",\"sha256\":\"{sha256}\",\
             \"sha256_ok\":{sha_ok},\"shared\":{shared},\"target_lines\":{lines},\"percent\":{percent},\
             \"not_a_lift\":{not_a_lift}}}",
            row.target, row.source
        ));
    }
    if failed { Err(log) } else { Ok(log) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "20463525d9775a18c1b237d2676f364f367175ab";
    const SHA: &str = "2cb5bbfe21d6bf5438283476ed82fd291c7461d888f5877ba221d7254c13b4e3";
    const TARGET: &str = "crates/x/src/a.rs";
    const ORIGINAL: &str = "crates/x/src/b.rs";

    fn lift() -> Lift {
        Lift {
            source: "crates/fs-a/src/a.rs".to_string(),
            commit: COMMIT.to_string(),
            sha256_at_lift: SHA.to_string(),
            licence: UPSTREAM_LICENCE.to_string(),
            target: TARGET.to_string(),
            guard_test: "a::tests::test_guard".to_string(),
        }
    }

    fn analogue() -> Analogue {
        Analogue {
            target: ORIGINAL.to_string(),
            upstream: "crates/fs-b/src/lib.rs".to_string(),
            note: "same concept".to_string(),
        }
    }

    fn header(licence: &str, changes: &str, left_out: &str) -> String {
        format!(
            "{PROVENANCE}crates/fs-a/src/a.rs @ {COMMIT}; licence {licence}; Changes: {changes}; \
             Left out: {left_out}"
        )
    }

    fn body() -> &'static str {
        "\npub fn a() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_guard() {}\n}\n"
    }

    fn good_file() -> String {
        format!("{}{}", header(UPSTREAM_LICENCE, "x", "y"), body())
    }

    /// The errors for `row` with the given target text, beside a well-formed analogue.
    fn errors(row: &Lift, text: Option<String>) -> Vec<String> {
        let read = |rel: &str| {
            if rel == TARGET {
                text.clone()
            } else {
                Some(format!(
                    "{ORIGINAL_HEADER}crates/fs-b/src/lib.rs @ 2046 (not copied).\n"
                ))
            }
        };
        check_rows(std::slice::from_ref(row), &[analogue()], &read, &[])
    }

    fn fails_with(found: &[String], needle: &str) {
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains(needle), "{found:?}");
    }

    #[test]
    fn test_well_formed_rows_pass() {
        assert_eq!(errors(&lift(), Some(good_file())), Vec::<String>::new());
    }

    #[test]
    fn test_rule_a_missing_target_fails() {
        fails_with(&errors(&lift(), None), "lift target is missing");
        let read = |_: &str| None;
        let found = check_rows(&[], &[analogue()], &read, &[]);
        fails_with(&found, "analogue target is missing");
    }

    #[test]
    fn test_rule_b_header_fields() {
        let check = |text: String| errors(&lift(), Some(format!("{text}{}", body())));
        fails_with(
            &check("//! Determinism: x".to_string()),
            "does not start with",
        );
        let other = format!("{PROVENANCE}crates/fs-a/src/lib.rs @ {COMMIT}; licence x");
        fails_with(&check(other), "does not start with");
        let no_left_out = format!(
            "{PROVENANCE}crates/fs-a/src/a.rs @ {COMMIT}; licence {UPSTREAM_LICENCE}; Changes: x"
        );
        fails_with(&check(no_left_out), "header lacks");
        fails_with(&check(header("MIT", "x", "y")), "header licence MIT");
        fails_with(&check(header(UPSTREAM_LICENCE, " ", "y")), "empty");
        fails_with(&check(header(UPSTREAM_LICENCE, "x", "")), "empty");
    }

    #[test]
    fn test_rule_c_guard_test_must_be_in_the_test_module() {
        let text = format!(
            "{}\nfn test_guard() {{}}\n#[cfg(test)]\nmod tests {{}}\n",
            header(UPSTREAM_LICENCE, "x", "y")
        );
        fails_with(&errors(&lift(), Some(text)), "is not in its test module");
        let row = Lift {
            guard_test: "a::tests::test_other".to_string(),
            ..lift()
        };
        fails_with(&errors(&row, Some(good_file())), "test_other");
    }

    #[test]
    fn test_rule_d_forms_and_licence() {
        let short = Lift {
            sha256_at_lift: "abc".to_string(),
            ..lift()
        };
        fails_with(&errors(&short, Some(good_file())), "not 64 hex");
        let upper = Lift {
            sha256_at_lift: SHA.to_uppercase(),
            ..lift()
        };
        fails_with(&errors(&upper, Some(good_file())), "not 64 hex");
        // A wrong id in row and header together agrees with itself, so the row is checked against upstream's.
        let wrong = Lift {
            licence: "MIT".to_string(),
            ..lift()
        };
        let text = format!("{}{}", header("MIT", "x", "y"), body());
        fails_with(&errors(&wrong, Some(text)), "is not upstream's");
        let bad_commit = Lift {
            commit: "2046".to_string(),
            ..lift()
        };
        let found = errors(&bad_commit, Some(good_file()));
        assert!(found[0].contains("not 40 hex"), "{found:?}");
    }

    #[test]
    fn test_rule_f_provenance_needs_a_row_and_analogues_say_original() {
        let read = |_: &str| Some(good_file());
        let orphan = vec!["crates/y/src/c.rs".to_string()];
        let found = check_rows(&[lift()], &[], &read, &orphan);
        fails_with(&found, "without a [[lift]] row");
        // An analogue still carrying a lift header is not an analogue.
        let found = check_rows(&[], &[analogue()], &read, &[]);
        fails_with(&found, "an analogue must open with");
        assert!(has_provenance_line(&format!("//! doc\n{PROVENANCE}x")));
        assert!(!has_provenance_line("//! Original FrankenMining code"));
    }

    #[test]
    fn test_parse_refuses_what_is_outside_the_schema() {
        let row = format!(
            "[[lift]]\nsource = \"s\"\ncommit = \"{COMMIT}\"\nsha256_at_lift = \"{SHA}\"\n\
             licence = \"{UPSTREAM_LICENCE}\"\ntarget = \"t\"\nguard_test = \"g\"\n"
        );
        let (lifts, analogues) =
            parse_lifts(&format!("schema_version = 2\n{row}")).expect("schema 2");
        assert_eq!((lifts.len(), analogues.len()), (1, 0));
        let e = parse_lifts(&format!("schema_version = 1\n{row}")).expect_err("schema 1");
        assert!(e.contains("schema_version = 2"), "{e}");
        let e =
            parse_lifts(&format!("schema_version = 2\n{row}extra = \"x\"\n")).expect_err("field");
        assert!(e.contains("unknown field extra"), "{e}");
        let e = parse_lifts("schema_version = 2\n[[other]]\nx = \"y\"\n").expect_err("table");
        assert!(e.contains("unknown table"), "{e}");
        let e = parse_lifts("schema_version = 2\n[[lift]]\nsource = \"s\"\n").expect_err("short");
        assert!(e.contains("lacks commit"), "{e}");
    }

    #[test]
    fn test_overlap_counts_distinct_long_trimmed_lines() {
        let long = "let shared_line_of_some_length = 1;";
        let target = format!("    {long}\n{long}\nshort\nlet only_in_the_target_file_here = 2;\n");
        assert_eq!(overlap(&target, &format!("\t{long}\n")), (1, 2));
        assert_eq!(overlap("", long), (0, 0));
    }

    /// Rule (e) against a real git repository: the digest and the overlap are read through `git show`.
    #[test]
    fn test_rule_e_upstream_digest_and_overlap() {
        // Removes the fixture even when an assertion fails: a `.git` left in an rch worker's temp dir, which sits
        // inside the project, makes a host that declares live checkouts refuse every later sync.
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let base = std::env::temp_dir().join(format!("fmine-lifts-{}", std::process::id()));
        let _cleanup = Cleanup(base.clone());
        let (up, ws) = (base.join("up"), base.join("ws"));
        let source_text = (0..10)
            .map(|i| format!("let upstream_line_number_{i} = {i};\n"))
            .collect::<String>();
        fs::create_dir_all(&up).expect("upstream dir");
        fs::write(up.join("a.rs"), &source_text).expect("source");
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&up)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .output()
                .expect("git");
            assert!(out.status.success(), "{args:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["add", "a.rs"]);
        git(&["commit", "-q", "-m", "x"]);
        let commit = git(&["rev-parse", "HEAD"]);
        let sha = fmine_core::hash::sha256(source_text.as_bytes()).to_hex();
        let registry = |sha: &str| {
            format!(
                "schema_version = 2\n[[lift]]\nsource = \"a.rs\"\ncommit = \"{commit}\"\n\
                 sha256_at_lift = \"{sha}\"\nlicence = \"{UPSTREAM_LICENCE}\"\ntarget = \"t.rs\"\n\
                 guard_test = \"g\"\n"
            )
        };
        fs::create_dir_all(ws.join("registries")).expect("registries");
        let write = |sha: &str, target: &str| {
            fs::write(ws.join(REGISTRY_PATH), registry(sha)).expect("registry");
            fs::write(ws.join("t.rs"), target).expect("target");
        };

        write(&sha, &source_text);
        let log = check_upstream(&ws, &up).expect("a true lift");
        assert_eq!(log.len(), 1, "{log:?}");
        assert!(
            log[0].contains("\"sha256_ok\":true,\"shared\":10,\"target_lines\":10,\"percent\":100"),
            "{log:?}"
        );

        write(&"0".repeat(64), &source_text);
        let log = check_upstream(&ws, &up).expect_err("wrong digest");
        assert!(log[0].contains("\"sha256_ok\":false"), "{log:?}");

        let fresh = (0..10)
            .map(|i| format!("let written_fresh_line_number_{i} = {i};\n"))
            .collect::<String>();
        write(&sha, &fresh);
        let log = check_upstream(&ws, &up).expect_err("not a lift");
        assert!(log[0].contains("\"not_a_lift\":true"), "{log:?}");
    }

    fn workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn test_committed_registry_passes_with_seven_lifts_and_five_analogues() {
        let root = workspace_root();
        check_lifts(&root).expect("committed lifts");
        let (lifts, analogues) = load(&root).expect("registry");
        assert_eq!((lifts.len(), analogues.len()), (7, 5));
    }

    /// Local only: set `FRANKENSIM_PATH` to a frankensim checkout that holds the pinned commit. The shared count
    /// is the known answer, because a ratio falls as a lifted file grows (det.rs: 82 lines, 54% of 152 at
    /// bd-5vex's measure, 41% of 198 now).
    #[test]
    fn test_upstream_known_answers_when_a_checkout_is_given() {
        let Ok(up) = std::env::var("FRANKENSIM_PATH") else {
            println!("skipped: FRANKENSIM_PATH is not set");
            return;
        };
        // One row is under MIN_OVERLAP_PERCENT, and that is its known answer: fs-sparse keeps precond.rs's tests in
        // tests/precond_battery.rs, so the target's test module shares no line with the source
        // (registries/lifts.toml). Every digest must still match, and no other row may be flagged.
        let log =
            check_upstream(&workspace_root(), Path::new(&up)).expect_err("precond.rs flagged");
        assert_eq!(log.len(), 7, "{log:?}");
        assert!(
            log.iter().all(|l| l.contains("\"sha256_ok\":true")),
            "{log:?}"
        );
        let flagged: Vec<&String> = log
            .iter()
            .filter(|l| l.contains("\"not_a_lift\":true"))
            .collect();
        assert_eq!(flagged.len(), 1, "{log:?}");
        assert!(
            flagged[0].contains("crates/fs-sparse/src/precond.rs")
                && flagged[0].contains("\"shared\":38,"),
            "{log:?}"
        );
        assert!(log[0].contains("\"shared\":82,"), "{log:?}");
        assert!(
            log[2].contains("crates/fs-rand/src/philox.rs") && log[2].contains(SHA),
            "{log:?}"
        );
        assert!(
            log[3].contains("crates/fs-evidence/src/color.rs"),
            "{log:?}"
        );
    }
}
