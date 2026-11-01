//! `check-int-as-001a`: the closure half of INT-AS-001 (§34.2, BD-325).
//!
//! asupersync is the suite's one async runtime. §27.5 takes it by git rev and fsqlite takes it from crates.io, so
//! without `[patch.crates-io]` the graph silently holds two packages, with two `Cx` types. On each production
//! closure this checks:
//! - A: one package id for each of [`SINGLE_ID`] (L-5);
//! - B(i): none of [`BANNED_RUNTIMES`], with the path that brought it in (criterion 9);
//! - B(ii): no detached worker or ambient timer in our own code, enforced by [`crate::check_caps_in_source`];
//! - C: asupersync, when present, comes through the patch at `franken-stack.lock`'s rev.
//!
//! A closure is `cargo tree` output. The checks are pure functions of that text, the root manifest and the stack
//! lock, so the fixtures exercise exactly what runs on the real workspace. Criteria 1 to 8 are INT-AS-001b.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use fmine_core::hash::sha256;

/// Crates whose types must have one identity across the graph.
pub const SINGLE_ID: [&str; 3] = ["asupersync", "fgdb-types", "fnx-classes"];
/// A second runtime. `futures-executor` carries a thread pool.
pub const BANNED_RUNTIMES: [&str; 6] = [
    "tokio",
    "async-std",
    "smol",
    "async-executor",
    "futures-executor",
    "actix-rt",
];
/// Production binaries whose closures are checked one by one, once the crate exists.
const POSTURE_ROOTS: [&str; 4] = ["fmine-cli", "fmine-oracles", "fmine-gen", "fmine-derive"];
const RUNTIME: &str = "asupersync";

/// One package in a `cargo tree --prefix depth --format {p}` listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeLine {
    pub depth: usize,
    pub name: String,
    pub version: String,
    /// The path or git source; `crates.io` for a package printed without one.
    pub source: String,
}

impl TreeLine {
    fn id(&self) -> String {
        format!("{} v{} ({})", self.name, self.version, self.source)
    }
}

/// Parses a listing line; `None` for the blank line between workspace roots.
#[must_use]
pub fn parse_tree_line(line: &str) -> Option<TreeLine> {
    let digits = line.find(|c: char| !c.is_ascii_digit())?;
    let depth = line[..digits].parse().ok()?;
    let mut words = line[digits..].split_whitespace();
    let name = words.next()?.to_string();
    let version = words.next()?.strip_prefix('v')?.to_string();
    let source = words
        .filter(|w| *w != "(*)" && *w != "(proc-macro)")
        .collect::<Vec<_>>()
        .join(" ");
    let source = source.trim_matches(['(', ')']);
    Some(TreeLine {
        depth,
        name,
        version,
        source: if source.is_empty() {
            "crates.io".to_string()
        } else {
            source.to_string()
        },
    })
}

/// One line of the check's log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRow {
    pub check: &'static str,
    pub status: &'static str,
    pub evidence: String,
}

impl CheckRow {
    /// `{"check":"A|B(i)|B(ii)|C","status":"pass|fail|not_applicable","evidence":…}`.
    #[must_use]
    pub fn log_line(&self) -> String {
        let evidence = self.evidence.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "{{\"check\":\"{}\",\"status\":\"{}\",\"evidence\":\"{evidence}\"}}",
            self.check, self.status
        )
    }
}

fn row(check: &'static str, errors: &[String], when_clear: &str) -> CheckRow {
    CheckRow {
        check,
        status: if errors.is_empty() { "pass" } else { "fail" },
        evidence: errors
            .first()
            .map_or_else(|| when_clear.to_string(), Clone::clone),
    }
}

/// The checks' results for one workspace state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub rows: Vec<CheckRow>,
    pub errors: Vec<String>,
    /// asupersync is in some closure.
    pub present: bool,
}

impl Report {
    /// The record's line, given `Cargo.lock`'s digest.
    #[must_use]
    pub fn status_line(&self, lock_sha256: &str) -> String {
        if !self.errors.is_empty() {
            format!("INT-AS-001a: FAIL ({} errors)", self.errors.len())
        } else if self.present {
            format!("INT-AS-001a: checks PASS at {lock_sha256}")
        } else {
            "INT-AS-001a: NOT_APPLICABLE (asupersync absent from Cargo.lock)".to_string()
        }
    }
}

/// `git_rev` of the `asupersync` row in `franken-stack.lock`.
fn pinned_rev(stack_lock: &str) -> Option<String> {
    let mut in_row = false;
    for line in stack_lock.lines() {
        if line.trim() == "[[foundation]]" {
            in_row = false;
        } else if crate::strip_kv_str(line, "name").as_deref() == Some(RUNTIME) {
            in_row = true;
        } else if in_row && let Some(rev) = crate::strip_kv_str(line, "git_rev") {
            return Some(rev);
        }
    }
    None
}

/// `rev` of `asupersync` under `[patch.crates-io]`, which §27.5 writes as one inline table.
fn patched_rev(manifest: &str) -> Option<String> {
    let mut in_patch = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_patch = line == "[patch.crates-io]";
        } else if in_patch && line.split('=').next().map(str::trim) == Some(RUNTIME) {
            let (_, tail) = line.split_once("rev")?;
            return tail.split('"').nth(1).map(str::to_string);
        }
    }
    None
}

/// Checks A, B(i) and C over `(posture, cargo tree text)` closures.
#[must_use]
pub fn evaluate(closures: &[(String, String)], manifest: &str, stack_lock: &str) -> Report {
    let (mut one_id, mut runtimes, mut patch) = (Vec::new(), Vec::new(), Vec::new());
    let mut runtime_sources = BTreeSet::new();
    for (posture, tree) in closures {
        let mut ids: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut path: Vec<String> = Vec::new();
        for line in tree.lines().filter_map(parse_tree_line) {
            path.truncate(line.depth);
            path.push(line.name.clone());
            if BANNED_RUNTIMES.contains(&line.name.as_str()) {
                runtimes.push(format!(
                    "{} in the closure of {posture}: {}",
                    line.name,
                    path.join(" -> ")
                ));
            }
            if let Some(name) = SINGLE_ID.iter().find(|n| **n == line.name) {
                ids.entry(name).or_default().insert(line.id());
            }
            if line.name == RUNTIME {
                runtime_sources.insert(line.source);
            }
        }
        for (name, found) in ids.iter().filter(|(_, found)| found.len() > 1) {
            let listed: Vec<&str> = found.iter().map(String::as_str).collect();
            one_id.push(format!(
                "{name}: {} package ids in closure of {posture}: {}",
                found.len(),
                listed.join("; ")
            ));
        }
    }
    let present = !runtime_sources.is_empty();
    if present {
        let pin = pinned_rev(stack_lock);
        match (patched_rev(manifest), &pin) {
            (None, _) => patch.push(format!("missing [patch.crates-io] {RUNTIME}")),
            (_, None) => patch.push(format!("no git_rev for {RUNTIME} in franken-stack.lock")),
            (Some(rev), Some(pin)) if rev != *pin => patch.push(format!(
                "[patch.crates-io] {RUNTIME} rev {rev} is not franken-stack.lock's {pin}"
            )),
            _ => {}
        }
        if let Some(pin) = pin {
            let wanted = format!("rev={pin}");
            for source in runtime_sources.iter().filter(|s| !s.contains(&wanted)) {
                patch.push(format!("{RUNTIME} from {source}, not the pinned rev {pin}"));
            }
        }
    }
    let mut rows = vec![
        row("A", &one_id, "one package id each"),
        row("B(i)", &runtimes, "no second runtime"),
        row("C", &patch, "patch and pin agree"),
    ];
    if !present {
        rows[2].status = "not_applicable";
        rows[2].evidence = format!("{RUNTIME} absent");
    }
    let errors = one_id.into_iter().chain(runtimes).chain(patch).collect();
    Report {
        rows,
        errors,
        present,
    }
}

pub(crate) fn cargo_tree(root: &Path, target: &[&str]) -> Result<String, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(root)
        .args(["tree", "--locked", "-e", "normal,build", "--target", "all"])
        .args(["--prefix", "depth", "--format", "{p}"])
        .args(target)
        .output()
        .map_err(|e| format!("cargo tree: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo tree {target:?}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("cargo tree {target:?}: {e}"))
}

/// Every production closure: the workspace without `xtask`, then each posture root that exists yet.
fn production_closures(root: &Path) -> Result<Vec<(String, String)>, String> {
    let all = ["--workspace", "--exclude", "xtask"];
    let mut closures = vec![("workspace".to_string(), cargo_tree(root, &all)?)];
    for posture in POSTURE_ROOTS {
        if root.join("crates").join(posture).exists() {
            let tree = cargo_tree(root, &["-p", posture])?;
            closures.push((posture.to_string(), tree));
        }
    }
    Ok(closures)
}

/// B(ii) over the real sources: `check_caps` reports the rest of L-3 itself.
fn worker_findings(root: &Path) -> Vec<String> {
    crate::check_caps(root)
        .err()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.contains("criterion 9"))
        .collect()
}

/// Runs the checks on the workspace at `root`. `Err` holds what failed, including a `cargo tree` that did not run.
pub fn check_int_as_001a(root: &Path) -> Result<(Report, String), Vec<String>> {
    let read = |name: &str| fs::read(root.join(name)).map_err(|e| vec![format!("{name}: {e}")]);
    let text = |name: &str| read(name).map(|b| String::from_utf8_lossy(&b).into_owned());
    let closures = production_closures(root).map_err(|e| vec![e])?;
    let mut report = evaluate(
        &closures,
        &text("Cargo.toml")?,
        &text("franken-stack.lock")?,
    );
    let workers = worker_findings(root);
    report.rows.insert(
        2,
        row("B(ii)", &workers, "no worker or timer outside the runtime"),
    );
    report.errors.extend(workers);
    let lock_sha256 = sha256(&read("Cargo.lock")?).to_hex();
    if report.errors.is_empty() {
        Ok((report, lock_sha256))
    } else {
        Err(report.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT: &str = "https://github.com/Dicklesworthstone/asupersync?rev=78b64636e#78b64636";
    const STACK: &str = "[[foundation]]\nid = \"FD-FSIM-00\"\nname = \"frankensim\"\ngit_rev = \"aaaaaaa\"\n\n\
                         [[foundation]]\nid = \"FD-AS-00\"\nname = \"asupersync\"\ngit_rev = \"78b64636e\"\n";
    const PATCHED: &str = "[workspace]\n\n[patch.crates-io]\nasupersync = { git = \"https://github.com/x\", \
                           rev = \"78b64636e\" }\n\n[profile.dev]\ndebug = 1\n";

    fn one(tree: &str) -> Vec<(String, String)> {
        vec![("fmine-cli".to_string(), tree.to_string())]
    }

    fn pinned_tree() -> String {
        format!("0fmine-cli v0.1.0 (/ws/crates/fmine-cli)\n1asupersync v0.5.0 ({GIT})\n")
    }

    #[test]
    fn test_parse_tree_line_git_registry_path_and_suffixes() {
        let got = parse_tree_line(&format!("12asupersync v0.5.0 ({GIT})")).expect("git");
        assert_eq!(
            (got.depth, got.name.as_str(), got.version.as_str()),
            (12, "asupersync", "0.5.0")
        );
        assert_eq!(got.source, GIT);
        // crates.io packages print no source, and a repeat carries (*).
        let got = parse_tree_line("1tokio v1.40.0 (*)").expect("registry");
        assert_eq!(
            (got.name.as_str(), got.source.as_str()),
            ("tokio", "crates.io")
        );
        let got = parse_tree_line("1serde_derive v1.0.200 (proc-macro) (*)").expect("macro");
        assert_eq!(got.source, "crates.io");
        let got = parse_tree_line("0fmine-core v0.1.0 (/ws/crates/fmine-core) (*)").expect("path");
        assert_eq!(got.source, "/ws/crates/fmine-core");
        let got = parse_tree_line("1x v1.0.0 (registry+https://example.org/index)").expect("alt");
        assert_eq!(got.source, "registry+https://example.org/index");
        assert_eq!(parse_tree_line(""), None);
        assert_eq!(parse_tree_line("not a line"), None);
    }

    /// The bead's worked example: fsqlite's crates.io asupersync beside the pinned one, with no patch.
    #[test]
    fn test_two_ids_and_no_patch_then_the_patch_clears_both() {
        let tree = format!(
            "0fmine-runtime v0.1.0 (/ws/crates/fmine-runtime)\n1asupersync v0.5.0 ({GIT})\n\
             1fsqlite-core v0.4.12\n2asupersync v0.5.0\n"
        );
        let report = evaluate(&one(&tree), "[workspace]\n", STACK);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.starts_with("asupersync: 2 package ids in closure of fmine-cli")),
            "{:?}",
            report.errors
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| e == "missing [patch.crates-io] asupersync")
        );
        assert_eq!(report.rows[0].status, "fail");
        assert_eq!(report.rows[2].status, "fail");
        assert_eq!(report.status_line("sha"), "INT-AS-001a: FAIL (3 errors)");

        let fixed = evaluate(&one(&pinned_tree()), PATCHED, STACK);
        assert_eq!(fixed.errors, Vec::<String>::new());
        assert_eq!(fixed.status_line("abc"), "INT-AS-001a: checks PASS at abc");
    }

    #[test]
    fn test_a_counts_each_single_id_crate_within_one_closure() {
        let tree = "0a v1.0.0 (/ws/a)\n1fnx-classes v1.0.0\n1fnx-classes v2.0.0\n";
        let report = evaluate(&one(tree), "", STACK);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(report.errors[0].starts_with("fnx-classes: 2 package ids"));
        // The same id twice, and a repeat marked (*), are one package.
        let tree = "0a v1.0.0 (/ws/a)\n1fnx-classes v1.0.0\n1b v1.0.0\n2fnx-classes v1.0.0 (*)\n";
        assert_eq!(evaluate(&one(tree), "", STACK).errors, Vec::<String>::new());
    }

    #[test]
    fn test_second_runtime_fails_with_the_path_that_brought_it_in() {
        let tree = "0fmine-cli v0.1.0 (/ws/crates/fmine-cli)\n1wrapper v1.0.0\n2tokio v1.40.0\n\
                    1other v1.0.0\n";
        let report = evaluate(&one(tree), "", STACK);
        assert_eq!(
            report.errors,
            ["tokio in the closure of fmine-cli: fmine-cli -> wrapper -> tokio"]
        );
        assert_eq!(report.rows[1].status, "fail");
        assert!(!report.present);
        for runtime in BANNED_RUNTIMES {
            let tree = format!("0a v1.0.0 (/ws/a)\n1{runtime} v1.0.0\n");
            assert_eq!(
                evaluate(&one(&tree), "", STACK).errors.len(),
                1,
                "{runtime}"
            );
        }
        // A name that merely starts like one is not one.
        let tree = "0a v1.0.0 (/ws/a)\n1tokio-macros-lookalike v1.0.0\n";
        assert_eq!(evaluate(&one(tree), "", STACK).errors, Vec::<String>::new());
    }

    #[test]
    fn test_c_refuses_a_patch_rev_or_source_off_the_pin() {
        let other_rev = PATCHED.replace("78b64636e", "0000000aa");
        let report = evaluate(&one(&pinned_tree()), &other_rev, STACK);
        assert_eq!(
            report.errors,
            ["[patch.crates-io] asupersync rev 0000000aa is not franken-stack.lock's 78b64636e"]
        );
        // The patch is right but the resolved package is not the patched one.
        let tree = "0fmine-cli v0.1.0 (/ws/crates/fmine-cli)\n1asupersync v0.5.0\n";
        let report = evaluate(&one(tree), PATCHED, STACK);
        assert_eq!(
            report.errors,
            ["asupersync from crates.io, not the pinned rev 78b64636e"]
        );
        // A patch for another crate, or the same key in another table, is no patch.
        let misplaced = "[dependencies]\nasupersync = { rev = \"78b64636e\" }\n";
        let report = evaluate(&one(&pinned_tree()), misplaced, STACK);
        assert_eq!(report.errors, ["missing [patch.crates-io] asupersync"]);
        // The other row's git_rev is not the runtime's.
        let no_pin = "[[foundation]]\nname = \"frankensim\"\ngit_rev = \"78b64636e\"\n";
        let report = evaluate(&one(&pinned_tree()), PATCHED, no_pin);
        assert_eq!(
            report.errors,
            ["no git_rev for asupersync in franken-stack.lock"]
        );
    }

    #[test]
    fn test_absent_runtime_is_not_applicable_and_logs_each_check() {
        let report = evaluate(&one("0a v1.0.0 (/ws/a)\n\n0b v1.0.0 (/ws/b)\n"), "", "");
        assert_eq!(
            report.status_line("sha"),
            "INT-AS-001a: NOT_APPLICABLE (asupersync absent from Cargo.lock)"
        );
        assert_eq!(
            report.rows[2].log_line(),
            "{\"check\":\"C\",\"status\":\"not_applicable\",\"evidence\":\"asupersync absent\"}"
        );
        let quoted = row("A", &["a \"b\" \\c".to_string()], "");
        assert!(
            quoted
                .log_line()
                .ends_with("\"evidence\":\"a \\\"b\\\" \\\\c\"}")
        );
    }

    /// The real workspace, through a real `cargo tree`, which is also what catches the listing's format drifting.
    #[test]
    fn test_workspace_is_not_applicable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let closures = production_closures(&root).expect("cargo tree");
        let packages: Vec<TreeLine> = closures[0].1.lines().filter_map(parse_tree_line).collect();
        assert!(
            packages
                .iter()
                .any(|p| p.depth == 0 && p.name == "fmine-cli")
        );
        assert!(
            packages.iter().all(|p| p.name != "xtask"),
            "xtask is not production"
        );
        let (report, lock) = check_int_as_001a(&root).expect("closure checks");
        assert!(report.status_line(&lock).contains("NOT_APPLICABLE"));
        assert_eq!(lock.len(), 64);
        for r in &report.rows {
            println!("{}", r.log_line());
        }
    }
}
