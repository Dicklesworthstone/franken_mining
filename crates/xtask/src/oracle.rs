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
//! - **Capture (BD-136).** `xtask capture-fixture` ([`capture`]) runs an oracle command and writes its stdout as the
//!   fixture and the record beside it, so the record's `command` and digests are those of the run that made the
//!   bytes. A fixture from a published source, with no command to run, still gets its record by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// What `xtask capture-fixture` records besides the command's output (§33.2, BD-136). Paths are relative to the
/// workspace root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capture {
    /// The fixture to write, under [`FIXTURES_DIR`].
    pub fixture: PathBuf,
    pub oracle: String,
    pub version: String,
    pub captured_by: String,
    pub licence: String,
    /// What the fixture is for, kept as a comment line at the top of the record. Optional.
    pub note: String,
    /// The files the command reads; at least one.
    pub inputs: Vec<PathBuf>,
    /// The command as argv, run from the root without a shell; its stdout is the fixture.
    pub command: Vec<String>,
    /// Write over an existing fixture and record. A new capture needs a bead, a reason and a reviewer (§33.2).
    pub replace: bool,
}

const CAPTURE_USAGE: &str = "usage: xtask capture-fixture <fixture> --oracle <s> --version <s> --captured-by <s> \
                             --licence <s> --input <path> [--input <path>]... [--note <s>] [--replace] -- <command> \
                             [<arg>]...";

/// `capture-fixture`'s arguments, after the subcommand. Values are checked by [`capture`].
pub fn parse_capture_args(args: &[String]) -> Result<Capture, String> {
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or_else(|| CAPTURE_USAGE.to_string())?;
    let (opts, command) = args.split_at(split);
    let mut c = Capture {
        command: command.iter().skip(1).cloned().collect(),
        ..Capture::default()
    };
    let mut fixture = None;
    let mut opts = opts.iter();
    while let Some(opt) = opts.next() {
        let slot = match opt.as_str() {
            "--replace" => {
                c.replace = true;
                continue;
            }
            "--oracle" => &mut c.oracle,
            "--version" => &mut c.version,
            "--captured-by" => &mut c.captured_by,
            "--licence" => &mut c.licence,
            "--note" => &mut c.note,
            "--input" => {
                let path = opts.next().ok_or_else(|| CAPTURE_USAGE.to_string())?;
                c.inputs.push(PathBuf::from(path));
                continue;
            }
            other if !other.starts_with("--") && fixture.is_none() => {
                fixture = Some(PathBuf::from(other));
                continue;
            }
            _ => return Err(CAPTURE_USAGE.to_string()),
        };
        *slot = opts
            .next()
            .ok_or_else(|| CAPTURE_USAGE.to_string())?
            .clone();
    }
    c.fixture = fixture.ok_or_else(|| CAPTURE_USAGE.to_string())?;
    Ok(c)
}

/// A relative path of plain components only.
fn is_plain(path: &Path) -> bool {
    path.components().count() > 0
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// The record's `inputs_digest`: one input's own sha256, or for several the sha256 of their manifest, the lines
/// `sha256:<hex> <path>` sorted by path, each ending in a newline. Also returns those lines.
pub fn inputs_digest(root: &Path, inputs: &[PathBuf]) -> Result<(String, Vec<String>), String> {
    let mut rows = Vec::new();
    for input in inputs {
        let shown = input.display().to_string();
        if !is_plain(input) || shown.contains(['\n', '\r']) {
            return Err(format!(
                "input {shown:?} must be a relative path without `..`"
            ));
        }
        let bytes = fs::read(root.join(input)).map_err(|e| format!("input {shown}: {e}"))?;
        rows.push((shown, sha256(&bytes).to_hex()));
    }
    rows.sort();
    if let Some(w) = rows.windows(2).find(|w| w[0].0 == w[1].0) {
        return Err(format!("input {} is named twice", w[0].0));
    }
    let lines: Vec<String> = rows
        .iter()
        .map(|(path, hex)| format!("sha256:{hex} {path}"))
        .collect();
    let digest = match rows.as_slice() {
        [] => return Err("capture-fixture needs at least one --input".to_string()),
        [(_, hex)] => format!("sha256:{hex}"),
        _ => {
            let manifest: String = lines.iter().map(|l| format!("{l}\n")).collect();
            format!("sha256:{}", sha256(manifest.as_bytes()).to_hex())
        }
    };
    Ok((digest, lines))
}

/// The argv as the record's `command`: an argument that is empty or holds whitespace is single-quoted.
fn shown_command(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.is_empty() || a.contains(char::is_whitespace) {
                format!("'{a}'")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_file(path: &Path, bytes: &[u8], replace: bool) -> Result<(), String> {
    use std::io::Write;
    let written = if replace {
        fs::write(path, bytes)
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut f| f.write_all(bytes))
    };
    written.map_err(|e| format!("{}: {e}", path.display()))
}

/// `xtask capture-fixture`: runs `c.command` from `root`, writes its stdout as the fixture and the nine-field
/// record beside it, and checks that record with [`record_errors`] first. Nothing is written when an argument is
/// refused, the command fails or prints nothing, or (without `replace`) the fixture or its record exists. Returns
/// `{"check":"capture-fixture","fixture","bytes","inputs_digest","outputs_digest"}`.
pub fn capture(
    root: &Path,
    c: &Capture,
    platform: &str,
    date: &str,
) -> Result<String, Vec<String>> {
    let mut errors = Vec::new();
    let shown = c.fixture.display().to_string();
    let name = c
        .fixture
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let under_fixtures =
        c.fixture.components().next() == Some(std::path::Component::Normal(FIXTURES_DIR.as_ref()));
    if !is_plain(&c.fixture) || !under_fixtures || c.fixture.components().count() < 2 {
        errors.push(format!(
            "fixture {shown:?} must be a relative path under {FIXTURES_DIR}/ without `..`"
        ));
    }
    if name.ends_with(RECORD_SUFFIX) {
        errors.push(format!("fixture {shown:?} is named like a capture record"));
    }
    let one_line = |v: &str| !v.trim().is_empty() && !v.contains(['"', '\\', '\n', '\r']);
    for (field, value) in [
        ("oracle", &c.oracle),
        ("version", &c.version),
        ("captured_by", &c.captured_by),
        ("licence", &c.licence),
        ("platform", &platform.to_string()),
    ] {
        if !one_line(value) {
            errors.push(format!(
                "{field} must be one non-empty line without `\"` or `\\`"
            ));
        }
    }
    if c.note.contains(['\n', '\r']) {
        errors.push("note must be one line".to_string());
    }
    if !crate::is_iso_date(date) {
        errors.push(format!("date {date:?} is not YYYY-MM-DD"));
    }
    if c.command.is_empty() {
        errors.push("capture-fixture needs a command after `--`".to_string());
    }
    if let Some(arg) = c
        .command
        .iter()
        .find(|a| a.contains(['"', '\\', '\'', '\n', '\r']))
    {
        errors.push(format!(
            "command argument {arg:?} holds a quote, a backslash or a line break"
        ));
    }
    let fixture = root.join(&c.fixture);
    let record_path = fixture.with_file_name(format!("{name}{RECORD_SUFFIX}"));
    if !c.replace {
        for path in [&fixture, &record_path] {
            if path.exists() {
                errors.push(format!(
                    "{} exists; a new capture needs --replace, a bead, a reason and a reviewer (§33.2)",
                    path.strip_prefix(root).unwrap_or(path).display()
                ));
            }
        }
    }
    let inputs = inputs_digest(root, &c.inputs)
        .map_err(|e| errors.push(e))
        .ok();
    let (Some((inputs_digest, lines)), Some((program, args)), true) =
        (inputs, c.command.split_first(), errors.is_empty())
    else {
        return Err(errors);
    };
    let command = shown_command(&c.command);
    let out = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| vec![format!("{program} failed to start: {e}")])?;
    if !out.status.success() {
        return Err(vec![format!(
            "`{command}` exited with {}; nothing written",
            out.status
        )]);
    }
    if out.stdout.is_empty() {
        return Err(vec![format!(
            "`{command}` printed nothing; nothing written"
        )]);
    }
    let outputs = sha256(&out.stdout).to_hex();
    let mut text =
        format!("# Capture record (§33.2) for {name}, written by `xtask capture-fixture`.\n");
    if !c.note.trim().is_empty() {
        text.push_str(&format!("# {}\n", c.note.trim()));
    }
    match lines.as_slice() {
        [one] => text.push_str(&format!(
            "# inputs_digest is the sha256 of {}.\n",
            one.split_once(' ').map_or("", |(_, p)| p)
        )),
        _ => {
            text.push_str(
                "# inputs_digest is the sha256 of these lines, each ending in a newline:\n",
            );
            for line in &lines {
                text.push_str(&format!("#   {line}\n"));
            }
        }
    }
    let outputs_digest = format!("sha256:{outputs}");
    for (field, value) in [
        ("oracle", c.oracle.as_str()),
        ("version", c.version.as_str()),
        ("platform", platform),
        ("inputs_digest", inputs_digest.as_str()),
        ("outputs_digest", outputs_digest.as_str()),
        ("command", command.as_str()),
        ("captured_by", c.captured_by.as_str()),
        ("date", date),
        ("licence", c.licence.as_str()),
    ] {
        text.push_str(&format!("{field} = \"{value}\"\n"));
    }
    let problems = record_errors(&shown, &text, &outputs);
    if !problems.is_empty() {
        return Err(problems);
    }
    if let Some(dir) = fixture.parent() {
        fs::create_dir_all(dir).map_err(|e| vec![format!("{}: {e}", dir.display())])?;
    }
    write_file(&fixture, &out.stdout, c.replace).map_err(|e| vec![e])?;
    write_file(&record_path, text.as_bytes(), c.replace).map_err(|e| vec![e])?;
    Ok(format!(
        "{{\"check\":\"capture-fixture\",\"fixture\":\"{shown}\",\"bytes\":{},\"inputs_digest\":\"{inputs_digest}\",\
         \"outputs_digest\":\"{outputs_digest}\"}}",
        out.stdout.len()
    ))
}

/// Where this process runs, as `<arch>-<os>`, for a record's `platform`.
pub fn host_platform() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

/// Today's UTC date as `YYYY-MM-DD`, for a record's `date`.
pub fn utc_date_today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let d = fmine_time::CivilDate::from_epoch_days(i64::try_from(secs / 86_400).unwrap_or(0));
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn capture_of(fixture: &str, inputs: &[&str], command: &[&str]) -> Capture {
        Capture {
            fixture: PathBuf::from(fixture),
            oracle: "printf".to_string(),
            version: "system printf".to_string(),
            captured_by: "test".to_string(),
            licence: "CC0-1.0".to_string(),
            note: String::new(),
            inputs: inputs.iter().map(PathBuf::from).collect(),
            command: command.iter().map(ToString::to_string).collect(),
            replace: false,
        }
    }

    /// BD-136: the tool's record passes the gate, names the command as run and the input's own sha256, and a second
    /// capture of the same fixture needs `replace`.
    #[test]
    fn test_capture_writes_a_record_the_gate_accepts() {
        let (root, _c) = scratch("capture");
        write(&root.join("gen.txt"), "abc");
        let c = Capture {
            note: " The bytes a test expects. ".to_string(),
            ..capture_of(
                "fixtures/t/out.txt",
                &["gen.txt"],
                &["printf", "fixture bytes"],
            )
        };
        let log = capture(&root, &c, "x86_64-linux", "2026-11-01").expect("captured");
        let fixture = root.join("fixtures/t/out.txt");
        assert_eq!(fs::read(&fixture).expect("fixture"), b"fixture bytes");
        let digest = sha256(b"fixture bytes").to_hex();
        assert_eq!(
            log,
            format!(
                "{{\"check\":\"capture-fixture\",\"fixture\":\"fixtures/t/out.txt\",\"bytes\":13,\
                 \"inputs_digest\":\"sha256:{ABC}\",\"outputs_digest\":\"sha256:{digest}\"}}"
            )
        );
        assert_eq!(check_capture_records(&root), Ok(1));
        let record = fs::read_to_string(root.join(format!("fixtures/t/out.txt{RECORD_SUFFIX}")))
            .expect("record");
        for line in [
            "command = \"printf 'fixture bytes'\"",
            "platform = \"x86_64-linux\"",
            "date = \"2026-11-01\"",
            "# The bytes a test expects.",
            "# inputs_digest is the sha256 of gen.txt.",
        ] {
            assert!(record.lines().any(|l| l == line), "{line}: {record}");
        }

        let again = capture(&root, &c, "x86_64-linux", "2026-11-02").expect_err("exists");
        assert_eq!(again.len(), 2, "{again:?}");
        assert!(
            again[0].starts_with("fixtures/t/out.txt exists"),
            "{again:?}"
        );
        assert!(
            again[1].starts_with(&format!("fixtures/t/out.txt{RECORD_SUFFIX} exists")),
            "{again:?}"
        );
        assert_eq!(fs::read(&fixture).expect("unchanged"), b"fixture bytes");

        let replaced = Capture {
            command: vec!["printf".to_string(), "other".to_string()],
            replace: true,
            ..c
        };
        capture(&root, &replaced, "x86_64-linux", "2026-11-02").expect("replaced");
        assert_eq!(fs::read(&fixture).expect("fixture"), b"other");
        assert_eq!(check_capture_records(&root), Ok(1));
    }

    /// A command that fails or prints nothing, and every refused argument, writes nothing.
    #[test]
    fn test_capture_refusals_write_nothing() {
        let (root, _c) = scratch("capture-refusals");
        write(&root.join("gen.txt"), "abc");
        let ok = capture_of("fixtures/x.txt", &["gen.txt"], &["printf", "x"]);
        let cases: Vec<(Capture, &str, &str)> = vec![
            (
                capture_of("fixtures/x.txt", &["gen.txt"], &["false"]),
                "2026-11-01",
                "`false` exited with",
            ),
            (
                capture_of("fixtures/x.txt", &["gen.txt"], &["true"]),
                "2026-11-01",
                "`true` printed nothing",
            ),
            (
                capture_of("elsewhere/x.txt", &["gen.txt"], &["printf", "x"]),
                "2026-11-01",
                "under fixtures/",
            ),
            (
                capture_of("fixtures/../x.txt", &["gen.txt"], &["printf", "x"]),
                "2026-11-01",
                "under fixtures/",
            ),
            (
                capture_of("fixtures", &["gen.txt"], &["printf", "x"]),
                "2026-11-01",
                "under fixtures/",
            ),
            (
                capture_of(
                    &format!("fixtures/x{RECORD_SUFFIX}"),
                    &["gen.txt"],
                    &["printf", "x"],
                ),
                "2026-11-01",
                "named like a capture record",
            ),
            (
                capture_of("fixtures/x.txt", &[], &["printf", "x"]),
                "2026-11-01",
                "at least one --input",
            ),
            (
                capture_of("fixtures/x.txt", &["missing.txt"], &["printf", "x"]),
                "2026-11-01",
                "input missing.txt",
            ),
            (
                capture_of("fixtures/x.txt", &["../gen.txt"], &["printf", "x"]),
                "2026-11-01",
                "relative path",
            ),
            (
                capture_of("fixtures/x.txt", &["gen.txt", "gen.txt"], &["printf", "x"]),
                "2026-11-01",
                "named twice",
            ),
            (
                capture_of("fixtures/x.txt", &["gen.txt"], &[]),
                "2026-11-01",
                "a command after `--`",
            ),
            (
                capture_of("fixtures/x.txt", &["gen.txt"], &["printf", "a\\n"]),
                "2026-11-01",
                "a backslash",
            ),
            (
                Capture {
                    oracle: " ".to_string(),
                    ..ok.clone()
                },
                "2026-11-01",
                "oracle must be",
            ),
            (
                Capture {
                    licence: "say \"x\"".to_string(),
                    ..ok.clone()
                },
                "2026-11-01",
                "licence must be",
            ),
            (
                Capture {
                    captured_by: "a\nb".to_string(),
                    ..ok.clone()
                },
                "2026-11-01",
                "captured_by must be",
            ),
            (
                Capture {
                    note: "two\nlines".to_string(),
                    ..ok.clone()
                },
                "2026-11-01",
                "note must be one line",
            ),
            (ok.clone(), "2026-11-1", "is not YYYY-MM-DD"),
        ];
        for (c, date, want) in &cases {
            let err = capture(&root, c, "x86_64-linux", date).expect_err(want);
            assert!(err.iter().any(|e| e.contains(want)), "{want}: {err:?}");
            assert!(!root.join(FIXTURES_DIR).exists(), "{want}: wrote something");
        }
        let err = capture(&root, &ok, "two words\nplatform", "2026-11-01").expect_err("platform");
        assert!(err[0].contains("platform must be"), "{err:?}");
        assert!(!root.join(FIXTURES_DIR).exists());
    }

    /// Several inputs: the digest of their manifest, sorted by path, so the order given does not matter.
    #[test]
    fn test_inputs_digest_of_several_is_their_sorted_manifest() {
        let (root, _c) = scratch("capture-inputs");
        write(&root.join("a.txt"), "abc");
        write(&root.join("b/c.txt"), "abd");
        let manifest = format!(
            "sha256:{ABC} a.txt\nsha256:{} b/c.txt\n",
            sha256(b"abd").to_hex()
        );
        let want = format!("sha256:{}", sha256(manifest.as_bytes()).to_hex());
        for order in [["a.txt", "b/c.txt"], ["b/c.txt", "a.txt"]] {
            let inputs: Vec<PathBuf> = order.iter().map(PathBuf::from).collect();
            let (digest, lines) = inputs_digest(&root, &inputs).expect("digest");
            assert_eq!(digest, want);
            assert_eq!(lines.len(), 2);
            assert!(lines[0].ends_with(" a.txt"), "{lines:?}");
        }
        let (one, _) = inputs_digest(&root, &[PathBuf::from("a.txt")]).expect("one");
        assert_eq!(one, format!("sha256:{ABC}"));
    }

    #[test]
    fn test_parse_capture_args() {
        let args = |s: &[&str]| s.iter().map(ToString::to_string).collect::<Vec<_>>();
        let c = parse_capture_args(&args(&[
            "fixtures/x.txt",
            "--oracle",
            "CPython datetime",
            "--version",
            "3.15",
            "--captured-by",
            "lab runner",
            "--licence",
            "PSF-2.0",
            "--input",
            "a.py",
            "--input",
            "b.py",
            "--note",
            "PO-TIME-1 table",
            "--replace",
            "--",
            "python3",
            "-I",
            "a.py",
        ]))
        .expect("parsed");
        assert_eq!(
            c,
            Capture {
                fixture: PathBuf::from("fixtures/x.txt"),
                oracle: "CPython datetime".to_string(),
                version: "3.15".to_string(),
                captured_by: "lab runner".to_string(),
                licence: "PSF-2.0".to_string(),
                note: "PO-TIME-1 table".to_string(),
                inputs: vec![PathBuf::from("a.py"), PathBuf::from("b.py")],
                command: args(&["python3", "-I", "a.py"]),
                replace: true,
            }
        );
        for bad in [
            &["fixtures/x.txt", "--oracle", "o"][..],
            &["fixtures/x.txt", "--oracle", "--"][..],
            &["fixtures/x.txt", "--colour", "red", "--", "printf"][..],
            &["fixtures/x.txt", "fixtures/y.txt", "--", "printf"][..],
            &["--oracle", "o", "--", "printf"][..],
            &["fixtures/x.txt", "--input", "--", "printf"][..],
        ] {
            let e = parse_capture_args(&args(bad)).expect_err("usage");
            assert!(
                e.starts_with("usage: xtask capture-fixture"),
                "{bad:?}: {e}"
            );
        }
    }
}
