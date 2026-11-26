use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use fmine_core::rand::{PhiloxStream, RngKey};

#[test]
#[cfg(unix)]
fn non_unicode_argv_prints_one_usage_envelope() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["recon", "run", "--input"])
        .arg(OsStr::from_bytes(b"worksheet-\xff.toml"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.contains("\"status\":\"usage_error\""));
    assert!(stdout.contains("\"exit_code\":2"));
}

/// Verbs, flags and words `parse_args` knows, beside junk, so the generator reaches every branch of the parser
/// and the verbs behind it. `--workspace` values only ever name the test's empty or missing roots, so no issuer
/// file is read.
const TOKENS: &[&str] = &[
    "assure",
    "public",
    "challenges",
    "guidance",
    "completion",
    "track-record",
    "doctor",
    "robot-docs",
    "claim",
    "show",
    "why",
    "trace",
    "--synthetic",
    "--issuer",
    "barrick",
    "--through",
    "2025Q2",
    "2025Q5",
    "--site",
    "lumwana",
    "--as-of",
    "2026-02-30",
    "2026-01-15",
    "--format",
    "--json",
    "json",
    "markdown",
    "md",
    "--workspace",
    "--",
    "-",
    "",
    "--flag",
    "fmine.m0.claim.0",
    "\u{0}",
    "\"",
    "\\",
    "é",
];

/// The exit codes plan §30.1 defines.
fn in_dictionary(code: u8) -> bool {
    code == 0 || (2..=9).contains(&code)
}

fn scratch_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fmine-cli-exit-codes-{}-{name}",
        std::process::id()
    ))
}

fn check_envelope(args: &[String], root: &Path) -> u8 {
    let (text, code) = fmine_cli::run(args, root);
    assert!(
        in_dictionary(code),
        "exit {code} outside §30.1 for {args:?}"
    );
    let (_, format) = fmine_cli::invoke(args, root);
    if format == fmine_cli::OutputFormat::Json {
        assert!(
            text.starts_with("{\"schema_version\":\"fmine.env.v1\","),
            "{args:?}: {text}"
        );
        assert!(
            text.contains(&format!("\"exit_code\":{code},")),
            "{args:?}: {text}"
        );
        let status = match code {
            0 => "ok",
            2 => "usage_error",
            _ => "refused",
        };
        assert!(
            text.contains(&format!("\"status\":\"{status}\"")),
            "{args:?}: {text}"
        );
    }
    code
}

#[test]
fn test_seeded_argv_never_exits_outside_the_dictionary() {
    let missing = scratch_root("missing");
    let empty = scratch_root("empty");
    std::fs::create_dir_all(&empty).unwrap_or_else(|e| panic!("create {}: {e}", empty.display()));
    let roots = [missing.clone(), empty.clone()];
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 0x00B1_D112,
        partition: 0,
        domain: 0,
        var_group: 0,
        realization: 0,
        brick: 0,
    });
    // Cases per exit code, so a generator that only ever reaches usage errors fails instead of passing vacuously.
    let mut seen = [0u32; 256];
    for case in 0..400 {
        let mut draw = |n: usize| (rng.next_u64() % n as u64) as usize;
        // Half the cases edit a well-formed command (so they reach the verbs and their refusals); half are
        // free-form token strings.
        let mut args: Vec<String> = if case % 2 == 0 {
            SKELETONS[draw(SKELETONS.len())]
                .iter()
                .map(ToString::to_string)
                .collect()
        } else {
            Vec::new()
        };
        let edits = if case % 2 == 0 { draw(3) } else { draw(7) };
        for _ in 0..edits {
            let token = TOKENS[draw(TOKENS.len())].to_string();
            let at = draw(args.len() + 1);
            match draw(3) {
                0 if at < args.len() => args[at] = token,
                1 if at < args.len() => {
                    args.remove(at);
                }
                _ => args.insert(at, token),
            }
        }
        if let Some(at) = args.iter().position(|a| a == "--workspace")
            && draw(4) != 0
        {
            args.insert(at + 1, roots[draw(2)].display().to_string());
        }
        seen[usize::from(check_envelope(&args, &roots[case % 2]))] += 1;
    }
    let _ = std::fs::remove_dir(&empty);
    let refused: u32 = seen[3..=9].iter().sum();
    assert!(
        seen[0] > 0 && seen[2] > 0 && seen[3] > 0 && refused > seen[3],
        "ok {}, usage {}, refused {refused} (exit 3: {})",
        seen[0],
        seen[2],
        seen[3]
    );
}

/// One well-formed command per verb and selector. With no registry in the workspace, each issuer command refuses.
const SKELETONS: &[&[&str]] = &[
    &[],
    &["assure", "public", "--synthetic"],
    &["assure", "public", "--issuer", "barrick"],
    &["assure", "challenges", "--synthetic"],
    &["guidance", "completion", "--synthetic"],
    &["track-record", "--synthetic"],
    &[
        "track-record",
        "--issuer",
        "barrick",
        "--as-of",
        "2026-10-31",
    ],
    &[
        "guidance",
        "completion",
        "--issuer",
        "barrick",
        "--site",
        "lumwana",
        "--through",
        "2025Q2",
        "--as-of",
        "2026-01-15",
    ],
    &["doctor"],
    &["robot-docs"],
    &["claim", "show", "fmine.m0.claim.0"],
    &["claim", "why", "x", "--synthetic"],
    &["claim", "trace", "x", "--issuer", "barrick"],
];

#[test]
fn test_every_verb_with_no_registry_exits_inside_the_dictionary() {
    let missing = scratch_root("verbs");
    for verb in SKELETONS {
        let args: Vec<String> = verb.iter().map(ToString::to_string).collect();
        check_envelope(&args, &missing);
        let mut md = args.clone();
        md.extend(["--format".to_string(), "markdown".to_string()]);
        check_envelope(&md, &missing);
    }
}

/// A reader that has gone away: the binary must not panic (exit 101) when stdout is closed, as under
/// `fmine robot-docs | head -c 1`.
#[test]
fn test_closed_stdout_exits_5_with_a_line_on_stderr() {
    let (reader, writer) = io::pipe().unwrap_or_else(|e| panic!("pipe: {e}"));
    drop(reader);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn fmine: {e}"));
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr)
            .unwrap_or_else(|e| panic!("read stderr: {e}"));
    }
    let status = child.wait().unwrap_or_else(|e| panic!("wait fmine: {e}"));
    assert_eq!(status.code(), Some(5), "stderr: {stderr}");
    assert!(
        stderr.contains("fmine: the envelope could not be written to stdout"),
        "stderr: {stderr}"
    );
}

/// BD-114's smoke run: the built binary, given no verb, prints one triage envelope for the workspace, in JSON by
/// default and in Markdown on request.
#[test]
fn test_bare_fmine_prints_triage_for_the_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let json = "{\"schema_version\":\"fmine.env.v1\",\"verb\":\"triage\",\"status\":\"ok\",\"exit_code\":0,";
    for (extra, opening) in [
        (&[][..], json),
        (&["--format", "markdown"][..], "# fmine triage\n\n"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .arg("--workspace")
            .arg(&root)
            .args(extra)
            .output()
            .unwrap_or_else(|e| panic!("run fmine: {e}"));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(output.status.code(), Some(0), "{stdout}");
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.starts_with(opening), "{stdout}");
    }
}
