//! bd-ip99: `fmine plan pit` end to end. Every generated pair is also solved by brute force over all closed sets, so
//! the value checked here does not come from pitflow, its parser or its certificate.
//! The pairs are invented arithmetic, not mine data.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plan-pit-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn run(root: &Path, upit: &str, prec: &str, extra: &[&str]) -> Envelope {
    fs::write(root.join("pit.upit"), upit).unwrap();
    fs::write(root.join("pit.prec"), prec).unwrap();
    let mut call = args(&["plan", "pit", "--upit", "pit.upit", "--prec", "pit.prec"]);
    call.extend(args(extra));
    let (output, format) = invoke(&call, root);
    assert_eq!(format, OutputFormat::Json);
    output
}

fn string_field<'a>(json: &'a str, name: &str) -> &'a str {
    json.split_once(&format!("\"{name}\":\""))
        .unwrap_or_else(|| panic!("missing {name}: {json}"))
        .1
        .split('"')
        .next()
        .unwrap()
}

fn number_field(json: &str, name: &str) -> i64 {
    json.split_once(&format!("\"{name}\":"))
        .unwrap_or_else(|| panic!("missing {name}: {json}"))
        .1
        .split([',', '}'])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn upit(values: &[String]) -> String {
    let mut text = format!(
        "NAME: synthetic\nTYPE: UPIT\nNBLOCKS: {}\nOBJECTIVE_FUNCTION:\n",
        values.len()
    );
    for (id, value) in values.iter().enumerate() {
        text.push_str(&format!("{id} {value}\n"));
    }
    text.push_str("EOF\n");
    text
}

fn prec(requires: &[Vec<usize>]) -> String {
    requires
        .iter()
        .enumerate()
        .map(|(id, list)| {
            let tail: String = list.iter().map(|p| format!(" {p}")).collect();
            format!("{id} {}{tail}\n", list.len())
        })
        .collect()
}

/// Hundredths as `"-3.07"`.
fn hundredths(units: i64) -> String {
    let sign = if units < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", units.abs() / 100, units.abs() % 100)
}

/// The best value over every closed set, by enumeration, and the largest-numbered set (as a bit mask) with it.
fn brute_force(weights: &[i64], requires: &[Vec<usize>]) -> (i64, u32) {
    (0u32..1 << weights.len())
        .filter(|set| {
            (0..weights.len())
                .all(|b| (set >> b) & 1 == 0 || requires[b].iter().all(|&p| (set >> p) & 1 == 1))
        })
        .map(|set| {
            let value = (0..weights.len())
                .filter(|&b| (set >> b) & 1 == 1)
                .map(|b| weights[b])
                .sum();
            (value, set)
        })
        .max()
        .unwrap()
}

/// A seeded LCG: `next(bound)` is below `bound`.
fn lcg(seed: u64) -> impl FnMut(u64) -> u64 {
    let mut state = seed;
    move |bound| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % bound
    }
}

/// Forty pairs of 4 to 12 blocks: values in hundredths, each block requiring some lower-numbered blocks.
fn generated() -> Vec<(Vec<i64>, Vec<Vec<usize>>)> {
    let mut next = lcg(0x2545_f491_4f6c_dd1d);
    (0..40)
        .map(|case| {
            let n = 4 + (case % 9);
            let weights: Vec<i64> = (0..n).map(|_| next(4001) as i64 - 2200).collect();
            let requires: Vec<Vec<usize>> = (0..n)
                .map(|b| (0..b).filter(|_| next(3) == 0).collect())
                .collect();
            (weights, requires)
        })
        .collect()
}

#[test]
fn plan_pit_equals_brute_force_and_lists_a_closed_pit_of_that_value() {
    let mut nontrivial = 0;
    for (case, (weights, requires)) in generated().into_iter().enumerate() {
        let n = weights.len();
        let (expected, _) = brute_force(&weights, &requires);
        let values: Vec<String> = weights.iter().map(|&w| hundredths(w)).collect();
        let root = root();
        let output = run(
            &root,
            &upit(&values),
            &prec(&requires),
            &["--decimals", "2", "--list-blocks"],
        );
        let json = output.to_json();
        assert_eq!(
            (output.verb.as_str(), output.status, output.exit_code),
            ("plan pit", "ok", 0),
            "{json}"
        );
        assert_eq!(
            string_field(&json, "value"),
            hundredths(expected),
            "case {case}"
        );
        let listed: Vec<usize> = json
            .split_once("\"in_pit\":[")
            .unwrap()
            .1
            .split(']')
            .next()
            .unwrap()
            .split(',')
            .filter(|id| !id.is_empty())
            .map(|id| id.parse().unwrap())
            .collect();
        assert!(
            listed
                .iter()
                .all(|&b| requires[b].iter().all(|p| listed.contains(p)))
        );
        assert_eq!(listed.iter().map(|&b| weights[b]).sum::<i64>(), expected);
        assert_eq!(number_field(&json, "pit_blocks"), listed.len() as i64);
        assert_eq!(number_field(&json, "blocks"), n as i64);
        assert_eq!(
            number_field(&json, "arcs"),
            requires.iter().map(Vec::len).sum::<usize>() as i64
        );
        nontrivial += usize::from(!listed.is_empty() && listed.len() < n);
    }
    // Not every case may be all-or-nothing, or the closure constraint would go untested.
    assert!(nontrivial >= 20, "{nontrivial}");
}

/// A candidate file: a comment, then the ids split over a space-separated and a tab-separated line.
fn candidate_file(set: &[usize]) -> String {
    let ids: Vec<String> = set.iter().map(usize::to_string).collect();
    let (head, tail) = ids.split_at(ids.len() / 2);
    format!(
        "% candidate pit\n{}\n  {}\n",
        head.join(" "),
        tail.join("\t")
    )
}

#[test]
fn plan_pit_judges_candidates_as_brute_force_does() {
    let mut next = lcg(0x9e37_79b9_7f4a_7c15);
    let mut verdicts = std::collections::BTreeMap::new();
    for (weights, requires) in generated() {
        let n = weights.len();
        let values: Vec<String> = weights.iter().map(|&w| hundredths(w)).collect();
        let (optimum, best) = brute_force(&weights, &requires);
        let best: Vec<usize> = (0..n).filter(|&b| (best >> b) & 1 == 1).collect();
        let random: Vec<usize> = (0..n).filter(|_| next(2) == 0).collect();
        // The random set's closure: requirements point to lower numbers, so one downward sweep adds them all.
        let mut member = vec![false; n];
        for &b in &random {
            member[b] = true;
        }
        for b in (0..n).rev() {
            if member[b] {
                for &p in &requires[b] {
                    member[p] = true;
                }
            }
        }
        let closure: Vec<usize> = (0..n).filter(|&b| member[b]).collect();
        for set in [best, closure, random] {
            let unmet: Vec<(usize, usize)> = set
                .iter()
                .flat_map(|&b| {
                    requires[b]
                        .iter()
                        .filter(|p| !set.contains(p))
                        .map(move |&p| (b, p))
                })
                .collect();
            let value: i64 = set.iter().map(|&b| weights[b]).sum();
            let verdict = match (unmet.is_empty(), optimum - value) {
                (false, _) => "not_closed",
                (true, 0) => "optimal",
                (true, _) => "suboptimal",
            };
            let gap = if unmet.is_empty() {
                format!("\"{}\"", hundredths(optimum - value))
            } else {
                "null".into()
            };
            let witnesses: Vec<String> = unmet
                .iter()
                .take(10)
                .map(|(b, p)| format!("[{b},{p}]"))
                .collect();
            let root = root();
            fs::write(root.join("candidate.txt"), candidate_file(&set)).unwrap();
            let output = run(
                &root,
                &upit(&values),
                &prec(&requires),
                &["--decimals", "2", "--candidate", "candidate.txt"],
            );
            let json = output.to_json();
            assert_eq!(output.exit_code, 0, "{json}");
            let expected = format!(
                "\"blocks\":{},\"value\":\"{}\",\"unmet_requirements\":{},\"first_unmet\":[{}],\"gap_to_optimum\":{gap},\"verdict\":\"{verdict}\"}}",
                set.len(),
                hundredths(value),
                unmet.len(),
                witnesses.join(",")
            );
            assert!(json.contains(&expected), "missing {expected}: {json}");
            assert!(
                output
                    .rendered_markdown
                    .unwrap()
                    .contains(&format!("**Candidate** `candidate.txt`: {verdict}."))
            );
            *verdicts.entry(verdict).or_insert(0) += 1;
        }
    }
    // Each verdict must come up, or one branch goes untested.
    assert!(
        verdicts.len() == 3 && verdicts.values().all(|&count| count >= 5),
        "{verdicts:?}"
    );
    let root = root();
    for (text, detail) in [
        ("0 2", "line 1: block 2 is not one of 2 blocks"),
        ("1\n% 1\n1", "line 3: block 1 is listed twice"),
        ("0 1.0", "\"1.0\" is not a block id"),
    ] {
        fs::write(root.join("candidate.txt"), text).unwrap();
        let output = run(
            &root,
            &upit(&args(&["1", "-1"])),
            &prec(&[vec![], vec![0]]),
            &["--candidate", "candidate.txt"],
        );
        assert_eq!((output.status, output.exit_code), ("refused", 5));
        let refusal = output.refusal.unwrap();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains(detail), "{text:?}: {}", refusal.what);
    }
}

#[test]
fn plan_pit_binds_its_sources_and_matches_the_binary() {
    let root = root();
    // Block 3 (value 6.25) needs 0, 1 and 2 (−1.50 −2.00 −1.00): the pit is all four, worth 1.75.
    let upit_text = upit(&args(&["-1.50", "-2.00", "-1.00", "6.25", "-0.10"]));
    let prec_text = prec(&[vec![], vec![], vec![], vec![0, 1, 2], vec![3]]);
    let output = run(&root, &upit_text, &prec_text, &["--decimals", "2"]);
    let json = output.to_json();
    assert_eq!(output.exit_code, 0, "{json}");
    for expected in [
        "\"schema\":\"fmine.plan-pit.v1\"",
        "\"format\":\"minelib_upit\"",
        "\"name\":\"synthetic\"",
        "\"value\":\"1.75\"",
        "\"pit_blocks\":4",
        "\"rounding\":\"refuse\"",
        "\"rounded_values\":0",
        "\"value_bound\":\"0.000\"",
        "\"verified\":true",
    ] {
        assert!(json.contains(expected), "missing {expected}: {json}");
    }
    assert!(json.contains(&sha256(upit_text.as_bytes()).to_hex()));
    assert!(json.contains(&sha256(prec_text.as_bytes()).to_hex()));
    assert!(!json.contains("\"in_pit\""));
    assert!(output.warnings.is_empty() && output.anchor_id.is_none());
    assert!(output.no_claim.unwrap().contains("not a reserve"));
    assert_eq!(
        fs::read_to_string(root.join("pit.upit")).unwrap(),
        upit_text
    );
    let call = args(&[
        "plan",
        "pit",
        "--upit",
        "pit.upit",
        "--prec",
        "pit.prec",
        "--decimals",
        "2",
    ]);
    for (format, expected) in [
        ("json", json),
        ("markdown", output.rendered_markdown.unwrap()),
    ] {
        let result = Command::new(binary())
            .args(&call)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("{expected}\n")
        );
    }
}

#[test]
fn plan_pit_refuses_extra_places_unless_asked_and_then_rounds_away_from_zero() {
    let root = root();
    let text = upit(&args(&["2.0000005", "-0.0000005", "-1"]));
    let requires = prec(&[vec![], vec![0], vec![1]]);
    let refused = run(&root, &text, &requires, &[]);
    assert_eq!((refused.status, refused.exit_code), ("refused", 5));
    let refusal = refused.refusal.unwrap();
    assert_eq!(refusal.code, "FMINE-REF-506");
    assert!(refusal.what.contains("\"2.0000005\""), "{}", refusal.what);
    let output = run(&root, &text, &requires, &["--round", "--list-blocks"]);
    let json = output.to_json();
    assert_eq!(output.exit_code, 0, "{json}");
    // 2.000001 alone beats 2.000001 − 0.000001; both roundings went away from zero.
    for expected in [
        "\"value\":\"2.000001\"",
        "\"in_pit\":[0]",
        "\"rounding\":\"half_away_from_zero\"",
        "\"rounded_values\":2",
        "\"value_bound\":\"0.0000010\"",
    ] {
        assert!(json.contains(expected), "missing {expected}: {json}");
    }
    assert_eq!(output.warnings.len(), 1, "{:?}", output.warnings);
    assert!(output.warnings[0].starts_with("2 values were rounded to 6 places"));
}

#[test]
fn plan_pit_refuses_malformed_pairs_and_rejects_bad_options() {
    let root = root();
    let good_upit = upit(&args(&["1", "-1"]));
    let good_prec = prec(&[vec![], vec![0]]);
    for (upit_text, prec_text, detail) in [
        (
            good_upit.replace("TYPE: UPIT", "TYPE: CPIT"),
            good_prec.clone(),
            "not UPIT",
        ),
        (
            good_upit.replace("1 -1\n", "0 -1\n"),
            good_prec.clone(),
            "two values",
        ),
        (
            good_upit.replace("NBLOCKS: 2", "NBLOCKS: 3"),
            good_prec.clone(),
            "NBLOCKS 3",
        ),
        (
            good_upit.replace("1 -1\n", "1 1e3\n"),
            good_prec.clone(),
            "plain decimal",
        ),
        (good_upit.clone(), "0 0\n1 1 2\n".to_string(), "requires 2"),
        (
            good_upit.clone(),
            "0 0\n1 2 0\n".to_string(),
            "declares 2 but lists 1",
        ),
        (
            good_upit.clone(),
            "0 0\n1 2 0 0\n".to_string(),
            "required block twice",
        ),
        (good_upit.clone(), "0 0\n".to_string(), "no precedence line"),
        (
            good_upit.clone(),
            "0 0\n1 1 0\n1 1 0\n".to_string(),
            "two precedence lines",
        ),
    ] {
        let output = run(&root, &upit_text, &prec_text, &[]);
        assert_eq!(
            (output.status, output.exit_code),
            ("refused", 5),
            "{detail}: {}",
            output.to_json()
        );
        let refusal = output.refusal.unwrap();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains(detail), "{detail}: {}", refusal.what);
    }
    let missing = invoke(
        &args(&["plan", "pit", "--upit", "absent.upit", "--prec", "pit.prec"]),
        &root,
    )
    .0;
    assert_eq!((missing.status, missing.exit_code), ("refused", 5));
    for call in [
        vec!["plan", "pit", "--upit", "pit.upit"],
        vec![
            "plan", "pit", "--upit", "pit.upit", "--prec", "pit.prec", "--round", "--round",
        ],
        vec![
            "plan", "pit", "--upit", "a", "--upit", "b", "--prec", "pit.prec",
        ],
        vec![
            "plan",
            "pit",
            "--upit",
            "pit.upit",
            "--prec",
            "pit.prec",
            "--decimals",
            "10",
        ],
        vec![
            "plan",
            "pit",
            "--upit",
            "pit.upit",
            "--prec",
            "pit.prec",
            "--decimals",
            "-1",
        ],
        vec![
            "plan", "pit", "--upit", "pit.upit", "--prec", "pit.prec", "--scale", "6",
        ],
        vec!["plan", "pit", "--upit", "--prec", "pit.prec"],
        vec![
            "plan",
            "pit",
            "--upit",
            "pit.upit",
            "--prec",
            "pit.prec",
            "--candidate",
            "a",
            "--candidate",
            "b",
        ],
    ] {
        let output = invoke(&args(&call), &root).0;
        assert_eq!(
            (output.verb.as_str(), output.status, output.exit_code),
            ("plan pit", "usage_error", 2),
            "{call:?}"
        );
    }
}

#[test]
fn plan_pit_is_registered_and_documented() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let registry = fs::read_to_string(workspace.join("registries/verbs.toml")).unwrap();
    assert!(registry.contains("path = \"plan pit\""));
    let docs = invoke(&args(&["robot-docs"]), workspace).0;
    assert!(
        docs.payload_json
            .contains("plan pit --upit <file.upit> --prec <file.prec>")
    );
    assert!(docs.payload_json.contains("\"plan_pit\":{"));
}

/// GATE-PUSH-PIT1's table through the verb. MineLib's files are not in the repository (licence unrecorded): point
/// `FMINE_MINELIB_DIR` at `<name>/<name>.upit` and `.prec` copies and run with `--ignored`.
#[test]
#[ignore = "needs a local MineLib copy in FMINE_MINELIB_DIR"]
fn minelib_instances_match_gate_push_pit1_through_the_verb() {
    let dir = PathBuf::from(std::env::var_os("FMINE_MINELIB_DIR").expect("FMINE_MINELIB_DIR"));
    for (name, value, rounded, pit_blocks) in [
        ("newman1", "26086899.025970", 0, 1_059),
        ("kd", "652195036.914887", 0, 12_154),
        ("marvin", "1415655436.136541", 4_874, 8_516),
        ("mclaughlin", "1495886962.000000", 0, 110_624),
        ("mclaughlin_limit", "1495726474.000000", 0, 110_225),
        ("p4hd", "293373256.000000", 0, 28_844),
        ("sm2", "2743603730.300000", 0, 18_388),
        ("w23", "510973998.000000", 0, 49_061),
        ("zuck_large", "122220279.680000", 34, 96_821),
        ("zuck_medium", "1075124490.289275", 88, 27_387),
        ("zuck_small", "1422726897.734958", 30, 9_399),
    ] {
        let upit_path = format!("{name}/{name}.upit");
        let prec_path = format!("{name}/{name}.prec");
        let mut call = args(&[
            "plan",
            "pit",
            "--upit",
            &upit_path,
            "--prec",
            &prec_path,
            "--list-blocks",
        ]);
        if rounded > 0 {
            call.push("--round".into());
        }
        let output = invoke(&call, &dir).0;
        let json = output.to_json();
        assert_eq!(output.exit_code, 0, "{name}: {json}");
        assert_eq!(string_field(&json, "value"), value, "{name}");
        assert_eq!(number_field(&json, "rounded_values"), rounded, "{name}");
        assert_eq!(number_field(&json, "pit_blocks"), pit_blocks, "{name}");
        // The verb's own pit, read back as an untrusted candidate, is closed and optimal.
        let ids = json
            .split_once("\"in_pit\":[")
            .unwrap()
            .1
            .split(']')
            .next()
            .unwrap();
        let candidate = root().join("candidate.txt");
        fs::write(&candidate, ids.replace(',', "\n")).unwrap();
        call.extend(args(&["--candidate", candidate.to_str().unwrap()]));
        let checked = invoke(&call, &dir).0.to_json();
        let expected = format!(
            "\"blocks\":{pit_blocks},\"value\":\"{value}\",\"unmet_requirements\":0,\"first_unmet\":[],\"gap_to_optimum\":\"0.000000\",\"verdict\":\"optimal\"}}"
        );
        assert!(checked.contains(&expected), "{name}: missing {expected}");
        eprintln!(
            "{name}: value {value}, rounded {rounded}, pit {pit_blocks}; own pit as candidate: optimal"
        );
    }
}

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().unwrap();
    let profile = executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}
