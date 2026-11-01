//! Whole invocations of `fmine`: `fmine_cli::invoke`, which is `parse_args` then `execute_command` (plan §30.1 and
//! §33.4, PO-ADIT-4; BD-112).
//!
//! Input: bytes, each choosing one entry of a fixed vocabulary: a registered verb path (every `path` in
//! registries/verbs.toml, so a verb added there is fuzzed without editing this file), a flag the parsers know, a
//! flag with a value its parser accepts, or a value. The argument list is the chosen entries in order. Every invocation must exit with a code from the §30.1
//! dictionary (0, 2–9), and its envelope must be one line of JSON whose schema, exit code and status agree; in JSON
//! format that line is what `fmine` prints.
//!
//! Nothing outside the vocabulary reaches the parsers, and no entry holds a `/`, so no argument can name an
//! absolute path or a path through a directory. Each `--workspace` is followed by [`ROOT`], and [`ROOT`] is also the
//! default root. Every verb that creates or appends a file (journals, issued artifacts, exports) joins its path to
//! the workspace, so whatever it would write resolves under [`ROOT`], which does not exist, and fails. Free-form
//! text is fuzzed against `parse_args` alone, which executes nothing, in the `cli` target.
#![no_main]

use std::path::Path;
use std::sync::OnceLock;

use fmine_cli::{OutputFormat, invoke};
use fmine_fuzz::is_json_line;
use libfuzzer_sys::fuzz_target;

/// The workspace every invocation runs in: absent, and under a directory that does not exist either.
const ROOT: &str = "/nonexistent/fmine-fuzz-workspace";

/// The verb registry, read at compile time.
const VERBS: &str = include_str!("../../registries/verbs.toml");

/// Every option a verb parser in fmine-cli matches on.
const FLAGS: &[&str] = &[
    "--all-claims",
    "--artifact",
    "--artifact-format",
    "--as-of",
    "--asset",
    "--assessment",
    "--capacity-document",
    "--cell",
    "--claim",
    "--claims",
    "--code-revision",
    "--cohort",
    "--cohort-as-of",
    "--defect",
    "--definitions",
    "--digest",
    "--discover",
    "--expected-digest",
    "--expected-head",
    "--format",
    "--head",
    "--issuer",
    "--journal",
    "--json",
    "--key",
    "--ledger",
    "--left-document",
    "--left-issuer",
    "--max-age-ns",
    "--max-age-seconds",
    "--max-clock-skew-ms",
    "--max-skew-ns",
    "--metric",
    "--now",
    "--output",
    "--period",
    "--price-subject",
    "--provenance",
    "--purpose",
    "--reason",
    "--recipient-role",
    "--record",
    "--release-key",
    "--release-role",
    "--replacement",
    "--request",
    "--require-current",
    "--revision",
    "--right-document",
    "--right-issuer",
    "--role",
    "--selector",
    "--site",
    "--source-issuance",
    "--synthetic",
    "--through",
    "--trust-host-clock",
    "--tsa-roots",
    "--uncertainty-ns",
    "--workspace",
];

/// Each option that takes a value, with a value its parser accepts, as one entry. A command that gets past its
/// parser needs several options at once; one byte per option lets the fuzzer reach the verb behind the parser.
const PAIRS: &[(&str, &str)] = &[
    ("--artifact", "report.json"),
    ("--artifact-format", "json"),
    ("--as-of", "2026-10-31"),
    ("--as-of", "2026-10-31T00:00:00Z"),
    (
        "--assessment",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    ("--asset", "example-jv"),
    ("--capacity-document", "Report.pdf"),
    ("--cell", "demo"),
    ("--claim", "clm_00112233445566778899aabbccddeeff"),
    ("--claims", "clm_00112233445566778899aabbccddeeff"),
    (
        "--code-revision",
        "0123456789abcdef0123456789abcdef01234567",
    ),
    ("--cohort", "demo"),
    ("--cohort-as-of", "2026-10-31"),
    ("--defect", "code-defect"),
    ("--definitions", "table-identities.toml"),
    (
        "--digest",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "--expected-digest",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "--expected-head",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    ("--format", "markdown"),
    (
        "--head",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    ("--issuer", "barrick"),
    ("--issuer", "synthetic"),
    ("--journal", "journal"),
    ("--key", "key"),
    ("--ledger", "cohort.toml"),
    ("--left-document", "Left.pdf"),
    ("--left-issuer", "alpha"),
    ("--max-age-ns", "10000000000"),
    ("--max-age-seconds", "300"),
    ("--max-clock-skew-ms", "1"),
    ("--max-skew-ns", "0"),
    ("--metric", "sold"),
    ("--now", "2026-11-01T00:00:00Z"),
    ("--output", "report.md"),
    ("--period", "2025"),
    ("--price-subject", "barrick"),
    ("--provenance", "provenance.toml"),
    ("--purpose", "review"),
    ("--reason", "seeded-code-defect"),
    ("--recipient-role", "sponsor"),
    ("--record", "record"),
    ("--release-key", "release.key"),
    ("--release-role", "release-owner"),
    (
        "--replacement",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    ("--request", "request.toml"),
    ("--revision", "0123456789abcdef0123456789abcdef01234567"),
    ("--right-document", "Right.pdf"),
    ("--right-issuer", "beta"),
    ("--role", "sponsor"),
    ("--selector", "latest-as-of"),
    ("--site", "lumwana"),
    (
        "--source-issuance",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    ("--through", "2025Q2"),
    ("--tsa-roots", "trusted.pem"),
    ("--uncertainty-ns", "0"),
];

/// Values for those options, valid and not, and junk. None holds a `/`.
const VALUES: &[&str] = &[
    "synthetic",
    "barrick",
    "lumwana",
    "2025Q2",
    "2025Q5",
    "2026Q1",
    "FY2025",
    "2026-01-15",
    "2026-02-30",
    "2026-11-01T00:00:00Z",
    "2026-11-01T00:00:00.5Z",
    "1970-01-01T00:00:00Z",
    "0",
    "1",
    "5",
    "-1",
    "1e3",
    "18446744073709551615",
    "18446744073709551616",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "clm_00112233445566778899aabbccddeeff",
    "fmine.m0.claim.0",
    "release-owner",
    "sponsor",
    "reviewer-pool-7",
    "reviewer-pool-",
    "agent-session:00112233445566778899aabbccddeeff",
    "review",
    "json",
    "markdown",
    "md",
    "xml",
    "journal",
    "report",
    "key",
    "record",
    "roots.pem",
    "draft.md",
    "rules.toml",
    "corpus",
    "cell-one",
    ".",
    "..",
    "",
    "--",
    "-",
    "--flag",
    "\u{0}",
    "\n",
    "\"",
    "\\",
    "é",
];

/// The vocabulary: each verb path as its words, then each flag, each pair, and each value. At most 256 entries,
/// so every entry is one byte away.
fn vocabulary() -> &'static [Vec<String>] {
    static WORDS: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let verbs = VERBS.lines().filter_map(|line| {
            line.strip_prefix("path = \"")
                .and_then(|rest| rest.strip_suffix('"'))
                .map(|path| path.split(' ').map(str::to_owned).collect::<Vec<_>>())
        });
        let mut words: Vec<Vec<String>> = verbs.collect();
        assert!(words.len() >= 30, "verbs.toml lost its verb paths");
        words.extend(FLAGS.iter().map(|w| vec![(*w).to_owned()]));
        words.extend(
            PAIRS
                .iter()
                .map(|(f, v)| vec![(*f).to_owned(), (*v).to_owned()]),
        );
        words.extend(VALUES.iter().map(|w| vec![(*w).to_owned()]));
        assert!(words.len() <= 256, "{} vocabulary entries", words.len());
        assert!(
            words.iter().flatten().all(|w| !w.contains('/')),
            "a vocabulary entry holds a path separator"
        );
        words
    })
}

fuzz_target!(|data: &[u8]| {
    let words = vocabulary();
    let mut args: Vec<String> = Vec::with_capacity(data.len() + 4);
    for &byte in data.iter().take(64) {
        for word in &words[usize::from(byte) % words.len()] {
            args.push(word.clone());
            if word == "--workspace" {
                args.push(ROOT.to_owned());
            }
        }
    }
    let (env, format) = invoke(&args, Path::new(ROOT));
    let code = env.exit_code;
    assert!(
        code == 0 || (2..=9).contains(&code),
        "exit {code} outside the dictionary for {args:?}"
    );
    let json = env.to_json();
    assert!(
        is_json_line(&json),
        "not one line of JSON for {args:?}: {json:?}"
    );
    assert!(
        json.starts_with("{\"schema_version\":\"fmine.env.v1\","),
        "{args:?}: {json}"
    );
    assert!(
        json.contains(&format!("\"exit_code\":{code},")),
        "{args:?}: {json}"
    );
    let status = match code {
        0 => "ok",
        2 => "usage_error",
        _ => "refused",
    };
    assert!(
        json.contains(&format!("\"status\":\"{status}\"")),
        "{args:?}: {json}"
    );
    if format == OutputFormat::Markdown {
        assert!(
            env.rendered_markdown
                .as_ref()
                .is_none_or(|md| !md.is_empty()),
            "empty Markdown for {args:?}"
        );
    }
});
