//! GATE-ORACLE-01 (BD-013, §33.2): no oracle crate in a production closure, and a capture record beside every
//! fixture.
//!
//! - **Closure.** `cargo tree -e normal,build` over the workspace, without `xtask` and without the oracle crates
//!   that are members, names no [`ORACLE_CRATES`] package at any depth. `check_deps` reads only the members'
//!   manifests, so an oracle reached through a crate outside the workspace passes it; here it fails, with the path
//!   that brought it in. Dev-dependencies are not in the closure.
//! - **Capture records.** Every file under [`FIXTURES_DIR`] has a record `<file>.capture.toml` beside it, holding
//!   the nine §33.2 fields of [`CAPTURE_FIELDS`] once each. Its `outputs_digest` must be the fixture's sha256, so a
//!   fixture regenerated without a new record fails. A record with no fixture fails too. The form is
//!   `docs/templates/CAPTURE-RECORD.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;

use crate::int_as::parse_tree_line;

/// Oracle and verification crates (§33.2): never in a production closure.
pub const ORACLE_CRATES: [&str; 6] = [
    "fmine-oracles",
    "fmine-gen",
    "fmine-verify",
    "fmine-derive",
    "frankensympy",
    "fsym-core",
];

/// Where fixtures live (§33.2).
pub const FIXTURES_DIR: &str = "fixtures";

/// A fixture's record is the fixture's file name plus this suffix, in the same directory.
pub const RECORD_SUFFIX: &str = ".capture.toml";

/// §33.2's capture record: `{oracle, version, platform, inputs digest, outputs digest, command, captured_by, date,
/// licence}`.
pub const CAPTURE_FIELDS: [&str; 9] = [
    "oracle",
    "version",
    "platform",
    "inputs_digest",
    "outputs_digest",
    "command",
    "captured_by",
    "date",
    "licence",
];

/// What the gate measured when it passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Package lines in the production closure's listing.
    pub closure_packages: usize,
    /// Fixtures, each with a record.
    pub fixtures: usize,
}

impl Report {
    /// `{"check":"gate-oracle-01","closure_packages","fixtures","oracles_in_closure":0}`.
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"gate-oracle-01\",\"closure_packages\":{},\"fixtures\":{},\"oracles_in_closure\":0}}",
            self.closure_packages, self.fixtures
        )
    }
}

/// One finding per line of a `cargo tree --prefix depth --format {p}` listing that names an oracle crate.
pub fn closure_findings(tree: &str) -> Vec<String> {
    let mut path: Vec<String> = Vec::new();
    let mut findings = Vec::new();
    for line in tree.lines().filter_map(parse_tree_line) {
        path.truncate(line.depth);
        path.push(line.name.clone());
        if ORACLE_CRATES.contains(&line.name.as_str()) {
            findings.push(format!(
                "oracle crate {} in a production closure: {} (GATE-ORACLE-01)",
                line.name,
                path.join(" -> ")
            ));
        }
    }
    findings
}

/// The production closure's listing: the workspace without `xtask` and without the oracle crates among its members.
fn production_tree(root: &Path) -> Result<String, Vec<String>> {
    let mut args = vec!["--workspace".to_string()];
    for manifest in crate::workspace_member_manifests(root)? {
        let (name, _, _) = crate::parse_cargo_manifest(&manifest)?;
        if name == "xtask" || ORACLE_CRATES.contains(&name.as_str()) {
            args.push("--exclude".to_string());
            args.push(name);
        }
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    crate::int_as::cargo_tree(root, &args).map_err(|e| vec![e])
}

/// Every file under `dir`, in path order.
fn files_under(dir: &Path) -> Result<Vec<PathBuf>, Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = fs::read_dir(&d).map_err(|e| vec![format!("{}: {e}", d.display())])?;
        for entry in entries {
            let path = entry
                .map_err(|e| vec![format!("{}: {e}", d.display())])?
                .path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// What is wrong with one capture record, given its fixture's name and sha256 (hex).
pub fn record_errors(fixture: &str, record: &str, fixture_sha256: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let mut fields: BTreeMap<&str, String> = BTreeMap::new();
    for line in record
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let key = line.split_once('=').map(|(k, _)| k.trim());
        match key.and_then(|k| CAPTURE_FIELDS.iter().find(|f| **f == k)) {
            None => errors.push(format!(
                "{fixture}: record line `{line}` is not one of the §33.2 fields as `key = \"value\"`"
            )),
            Some(&field) => {
                let value = crate::strip_kv_str(line, field).unwrap_or_default();
                if fields.insert(field, value).is_some() {
                    errors.push(format!("{fixture}: record field {field} appears twice"));
                }
            }
        }
    }
    for field in CAPTURE_FIELDS {
        if fields.get(field).is_none_or(|v| v.trim().is_empty()) {
            errors.push(format!(
                "{fixture}: record field {field} is missing or empty"
            ));
        }
    }
    let is_digest = |v: &str| {
        v.strip_prefix("sha256:").is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    };
    for field in ["inputs_digest", "outputs_digest"] {
        if let Some(v) = fields.get(field).filter(|v| !v.is_empty() && !is_digest(v)) {
            errors.push(format!(
                "{fixture}: record field {field} `{v}` is not sha256:<64 lower-case hex>"
            ));
        }
    }
    if let Some(v) = fields.get("outputs_digest").filter(|v| is_digest(v))
        && v.as_str() != format!("sha256:{fixture_sha256}")
    {
        errors.push(format!(
            "{fixture}: outputs_digest {v} is not the fixture's sha256:{fixture_sha256}; a regenerated fixture \
             needs a new capture (§33.2)"
        ));
    }
    if let Some(v) = fields
        .get("date")
        .filter(|v| !v.is_empty() && !crate::is_iso_date(v))
    {
        errors.push(format!(
            "{fixture}: record field date `{v}` is not YYYY-MM-DD"
        ));
    }
    errors
}

/// Every fixture under [`FIXTURES_DIR`] against its record; the number of fixtures when all pass.
pub fn check_capture_records(root: &Path) -> Result<usize, Vec<String>> {
    let dir = root.join(FIXTURES_DIR);
    if !dir.exists() {
        return Ok(0);
    }
    let files = files_under(&dir)?;
    let listed: BTreeSet<&PathBuf> = files.iter().collect();
    let (mut errors, mut fixtures) = (Vec::new(), 0);
    for path in &files {
        let shown = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(fixture) = name.strip_suffix(RECORD_SUFFIX) {
            if !listed.contains(&path.with_file_name(fixture)) {
                errors.push(format!(
                    "{shown}: no fixture {fixture} beside this capture record"
                ));
            }
            continue;
        }
        fixtures += 1;
        let record = path.with_file_name(format!("{name}{RECORD_SUFFIX}"));
        let Ok(text) = fs::read_to_string(&record) else {
            errors.push(format!(
                "{shown}: no capture record {name}{RECORD_SUFFIX} beside it (§33.2)"
            ));
            continue;
        };
        match fs::read(path) {
            Ok(bytes) => errors.extend(record_errors(&shown, &text, &sha256(&bytes).to_hex())),
            Err(e) => errors.push(format!("{shown}: {e}")),
        }
    }
    if errors.is_empty() {
        Ok(fixtures)
    } else {
        Err(errors)
    }
}

/// Both halves of the gate over the workspace at `root`.
pub fn check(root: &Path) -> Result<Report, Vec<String>> {
    let tree = production_tree(root)?;
    let mut errors = closure_findings(&tree);
    let fixtures = check_capture_records(root).unwrap_or_else(|e| {
        errors.extend(e);
        0
    });
    if errors.is_empty() {
        Ok(Report {
            closure_packages: tree.lines().filter_map(parse_tree_line).count(),
            fixtures,
        })
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// sha256("abc"), FIPS 180-2 appendix B.1.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(tag: &str) -> (PathBuf, Cleanup) {
        let base = std::env::temp_dir().join(format!("fmine-oracle-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("scratch");
        (base.clone(), Cleanup(base))
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(path, text).expect("write");
    }

    fn record(outputs: &str) -> String {
        format!(
            "oracle = \"MineLib UPIT\"\nversion = \"2013-11\"\nplatform = \"x86_64-linux\"\n\
             inputs_digest = \"sha256:{}\"\noutputs_digest = \"{outputs}\"\ncommand = \"upit solve small.upit\"\n\
             captured_by = \"lab runner\"\ndate = \"2026-11-01\"\nlicence = \"CC-BY-4.0\"\n",
            "0".repeat(64)
        )
    }

    #[test]
    fn test_closure_findings_name_the_path_that_brought_the_oracle_in() {
        let tree = "0fmine-cli v0.1.0 (/ws/crates/fmine-cli)\n\
                    1fmine-public v0.1.0 (/ws/crates/fmine-public)\n\
                    2fs-bridge v0.3.0 (/up/fs-bridge)\n\
                    3fsym-core v0.2.0 (/up/fsym-core)\n\
                    1fmine-core v0.1.0 (/ws/crates/fmine-core)\n\
                    \n\
                    0fmine-io v0.1.0 (/ws/crates/fmine-io)\n\
                    1fmine-generic v1.0.0\n\
                    1fmine-gen v0.1.0 (/ws/crates/fmine-gen) (*)\n";
        assert_eq!(
            closure_findings(tree),
            [
                "oracle crate fsym-core in a production closure: fmine-cli -> fmine-public -> fs-bridge -> \
                 fsym-core (GATE-ORACLE-01)",
                "oracle crate fmine-gen in a production closure: fmine-io -> fmine-gen (GATE-ORACLE-01)",
            ]
        );
        let clean = "0fmine-cli v0.1.0 (/ws/crates/fmine-cli)\n1fmine-core v0.1.0 (/ws/crates/fmine-core)\n";
        assert!(closure_findings(clean).is_empty());
    }

    /// BD-013's done-when: an oracle crate in a production closure fails. The oracle comes in through a crate
    /// outside the workspace, so `check_deps`, which reads only the members' manifests, passes it. An oracle crate
    /// that is a member, and `xtask`, may depend on oracles: their closures are not production closures.
    #[test]
    fn test_seeded_oracle_outside_the_workspace_fails_the_closure_but_not_check_deps() {
        let (base, _c) = scratch("closure");
        let ws = base.join("ws");
        write(
            &ws.join("Cargo.toml"),
            "[workspace]\nmembers = [\n    \"crates/fmine-io\",\n    \"crates/fmine-gen\",\n    \"crates/xtask\",\n]\n\
             resolver = \"2\"\n",
        );
        write(
            &ws.join("deny.toml"),
            "[bans]\ndeny = [\n    { name = \"tokio\" },\n]\n",
        );
        let package = |name: &str, deps: &str| {
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n\n{deps}"
            )
        };
        write(
            &ws.join("crates/fmine-io/Cargo.toml"),
            &package(
                "fmine-io",
                "[dependencies]\nbridge = { path = \"../../../outside/bridge\" }\n",
            ),
        );
        write(&ws.join("crates/fmine-io/src/lib.rs"), "");
        for member in ["fmine-gen", "xtask"] {
            write(
                &ws.join(format!("crates/{member}/Cargo.toml")),
                &package(
                    member,
                    "[dependencies]\nfsym-core = { path = \"../../../outside/fsym-core\" }\n",
                ),
            );
            write(&ws.join(format!("crates/{member}/src/lib.rs")), "");
        }
        let bridge = base.join("outside/bridge/Cargo.toml");
        write(
            &bridge,
            &package(
                "bridge",
                "[dependencies]\nfsym-core = { path = \"../fsym-core\" }\n",
            ),
        );
        write(&base.join("outside/bridge/src/lib.rs"), "");
        write(
            &base.join("outside/fsym-core/Cargo.toml"),
            &package("fsym-core", ""),
        );
        write(&base.join("outside/fsym-core/src/lib.rs"), "");
        let lock = || {
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let out = Command::new(cargo)
                .current_dir(&ws)
                .args(["generate-lockfile", "--offline"])
                .output()
                .expect("cargo");
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        lock();
        assert!(crate::check_deps(&ws).is_ok(), "the direct check passes it");
        let err = check(&ws).expect_err("the closure holds fsym-core");
        assert_eq!(
            err,
            [
                "oracle crate fsym-core in a production closure: fmine-io -> bridge -> fsym-core (GATE-ORACLE-01)"
            ]
        );
        // As a dev-dependency of the outside crate it is not in the closure.
        write(
            &bridge,
            &package(
                "bridge",
                "[dev-dependencies]\nfsym-core = { path = \"../fsym-core\" }\n",
            ),
        );
        lock();
        let ok = check(&ws).expect("no oracle in the closure");
        assert_eq!(
            ok,
            Report {
                closure_packages: 2,
                fixtures: 0
            }
        );
        // The same gate fails a fixture with no capture record.
        write(&ws.join(FIXTURES_DIR).join("small.upit"), "abc");
        let err = check(&ws).expect_err("unrecorded fixture");
        assert_eq!(
            err,
            [format!(
                "fixtures/small.upit: no capture record small.upit{RECORD_SUFFIX} beside it (§33.2)"
            )]
        );
    }

    #[test]
    fn test_capture_record_fields_and_digest() {
        let good = record(&format!("sha256:{ABC}"));
        assert!(record_errors("f", &good, ABC).is_empty());
        // The fixture changed since its capture.
        let err = record_errors("f", &good, &"1".repeat(64));
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(err[0].contains("is not the fixture's sha256"), "{err:?}");
        let cases: [(String, &str); 8] = [
            (
                good.replace("licence = \"CC-BY-4.0\"\n", ""),
                "licence is missing or empty",
            ),
            (
                good.replace("CC-BY-4.0", " "),
                "licence is missing or empty",
            ),
            (
                format!("{good}oracle = \"again\"\n"),
                "oracle appears twice",
            ),
            (
                format!("{good}reviewer = \"x\"\n"),
                "`reviewer = \"x\"` is not one of the §33.2 fields",
            ),
            (
                good.replace(&format!("sha256:{ABC}"), ABC),
                "outputs_digest `ba78",
            ),
            (
                good.replace(&"0".repeat(64), &"A".repeat(64)),
                "inputs_digest `sha256:AAAA",
            ),
            (
                good.replace(&"0".repeat(64), &"0".repeat(63)),
                "inputs_digest `sha256:000",
            ),
            (
                good.replace("2026-11-01", "2026-1-01"),
                "date `2026-1-01` is not YYYY-MM-DD",
            ),
        ];
        for (text, want) in &cases {
            let err = record_errors("f", text, ABC);
            assert_eq!(err.len(), 1, "{want}: {err:?}");
            assert!(err[0].contains(want), "{want}: {err:?}");
        }
    }

    #[test]
    fn test_every_fixture_needs_a_record_with_its_digest() {
        let (root, _c) = scratch("fixtures");
        // No fixtures directory: nothing to check.
        assert_eq!(check_capture_records(&root), Ok(0));
        let dir = root.join(FIXTURES_DIR).join("pit");
        write(&dir.join("small.upit"), "abc");
        write(
            &dir.join(format!("small.upit{RECORD_SUFFIX}")),
            &record(&format!("sha256:{ABC}")),
        );
        assert_eq!(check_capture_records(&root), Ok(1));
        // Regenerated in place: the record no longer names it.
        write(&dir.join("small.upit"), "abd");
        let err = check_capture_records(&root).expect_err("regenerated");
        assert!(
            err[0].starts_with("fixtures/pit/small.upit: outputs_digest"),
            "{err:?}"
        );
        write(&dir.join("small.upit"), "abc");
        // A fixture with no record, and a record with no fixture.
        write(&dir.join("big.upit"), "abc");
        write(
            &root
                .join(FIXTURES_DIR)
                .join(format!("gone.txt{RECORD_SUFFIX}")),
            &record(&format!("sha256:{ABC}")),
        );
        let err = check_capture_records(&root).expect_err("two missing halves");
        assert_eq!(
            err,
            [
                format!(
                    "fixtures/gone.txt{RECORD_SUFFIX}: no fixture gone.txt beside this capture record"
                ),
                format!(
                    "fixtures/pit/big.upit: no capture record big.upit{RECORD_SUFFIX} beside it (§33.2)"
                ),
            ]
        );
    }
}
