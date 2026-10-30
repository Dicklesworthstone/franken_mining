//! The public forecast record's IO (BD-334.S1). `fmine_calib::record` is pure, so reading the files, writing the
//! digests and reading a time proof's dates happen here:
//! - `forecast-rules-hash <rules.toml>` writes `rules.sha256` beside the rules (their `FMRUL1` digest);
//! - `forecast-record-hash <dir>` validates `manifest.toml` against the source registries and the `rules.toml`
//!   beside it, then writes `record.sha256` (the `FMREC1` digest of `manifest.toml` and `forecasts.csv`);
//! - `forecast-record-verify <dir>` recomputes both digests, checks the RFC 3161 reply `record.tsr` against the
//!   certificate chain `tsa_chain.pem` with `openssl ts -verify`, reads the reply's time and accuracy, and
//!   validates the manifest against that interval;
//! - `forecast-rules-order` checks in git history that the FY2026 rules were committed before any forecast
//!   (BD-334.S2).
//!
//! A time comes only from a verified proof, never from a clock. A digest file is never overwritten with a
//! different digest: once stamped, a changed file is refused rather than re-hashed. The rules must parse and carry
//! no `OPEN` marker before they are hashed, and the record is judged against the dates its rules state.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use fmine_calib::record::{
    self, ProofTimes, RecordDates, RecordManifest, RecordRules, RegisteredDigest, format_utc,
    parse_hex32, parse_utc,
};

/// The FY2026 rules, relative to the workspace root.
pub const FY2026_RULES: &str = "forecasts/fy2026/rules.toml";
/// The FY2026 forecasts, relative to the workspace root; no commit may touch them before the rules exist.
pub const FY2026_FORECASTS: &str = "forecasts/fy2026/forecasts.csv";

type Res<T> = Result<T, Vec<String>>;

fn fail<T>(msg: String) -> Res<T> {
    Err(vec![msg])
}

fn read(path: &Path) -> Res<Vec<u8>> {
    fs::read(path).map_err(|e| vec![format!("cannot read {}: {e}", path.display())])
}

/// Writes `hex` to `path`, unless the file already holds a different digest.
fn write_digest(path: &Path, hex: &str) -> Res<()> {
    match fs::read_to_string(path) {
        Ok(old) if old.trim_end() == hex => return Ok(()),
        Ok(old) => {
            return fail(format!(
                "{} already holds {}, not {hex}: a hashed file changed; restore it or start a new record",
                path.display(),
                old.trim_end()
            ));
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return fail(format!("cannot read {}: {e}", path.display())),
    }
    fs::write(path, format!("{hex}\n"))
        .map_err(|e| vec![format!("cannot write {}: {e}", path.display())])
}

/// Every `(file, sha256)` in `registries/sources/*.toml`.
fn registered_digests(root: &Path) -> Res<Vec<(String, [u8; 32])>> {
    let dir = root.join("registries").join("sources");
    let entries =
        fs::read_dir(&dir).map_err(|e| vec![format!("cannot read {}: {e}", dir.display())])?;
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let reg = fmine_registry::sources::load_sources_registry(&p)
            .map_err(|e| vec![format!("{}: {e}", p.display())])?;
        for s in reg.sources {
            let Some(d) = parse_hex32(&s.sha256) else {
                return fail(format!(
                    "{}: {} has no 64-digit sha256",
                    p.display(),
                    s.file
                ));
            };
            out.push((s.file, d));
        }
    }
    Ok(out)
}

/// The rules, refused if any line, comments included, still carries an `OPEN` marker or they do not parse.
fn parse_rules_bytes(bytes: &[u8], name: &str) -> Res<RecordRules> {
    let text = std::str::from_utf8(bytes).map_err(|_| vec![format!("{name} is not UTF-8")])?;
    if let Some((n, _)) = text
        .lines()
        .enumerate()
        .find(|(_, l)| record::has_open_marker(l))
    {
        return fail(format!(
            "{name}: line {}: an OPEN point must be decided before the rules are stamped",
            n + 1
        ));
    }
    record::parse_rules(text).map_err(|e| vec![format!("{name}: {e}")])
}

struct Loaded {
    manifest: RecordManifest,
    dates: RecordDates,
    digest: String,
    rules_digest: String,
    registered: Vec<(String, [u8; 32])>,
}

/// Reads and checks everything that needs no proof: the manifest parses, it binds the rules file beside it and
/// names the rules' record, and it has no finding against the source registries.
fn load(root: &Path, dir: &Path) -> Res<Loaded> {
    let manifest_bytes = read(&dir.join("manifest.toml"))?;
    let csv = read(&dir.join("forecasts.csv"))?;
    let rules_bytes = read(&dir.join("rules.toml"))?;
    let text = String::from_utf8(manifest_bytes.clone())
        .map_err(|_| vec!["manifest.toml is not UTF-8".to_string()])?;
    let manifest =
        record::parse_manifest(&text).map_err(|e| vec![format!("manifest.toml: {e}")])?;
    let rules_digest = record::rules_digest(&rules_bytes);
    if rules_digest.0 != manifest.rules_sha256 {
        return fail(format!(
            "manifest.toml rules_sha256 differs from the FMRUL1 digest of rules.toml ({})",
            rules_digest.to_hex()
        ));
    }
    let rules = parse_rules_bytes(&rules_bytes, "rules.toml")?;
    if rules.record_id != manifest.record_id {
        return fail(format!(
            "manifest.toml record_id {:?} is not the rules' record {:?}",
            manifest.record_id, rules.record_id
        ));
    }
    let registered = registered_digests(root)?;
    let findings = findings(&manifest, &registered, None, rules.dates);
    if !findings.is_empty() {
        return Err(findings);
    }
    Ok(Loaded {
        digest: record::record_digest(&manifest_bytes, &csv).to_hex(),
        rules_digest: rules_digest.to_hex(),
        dates: rules.dates,
        manifest,
        registered,
    })
}

fn findings(
    m: &RecordManifest,
    registered: &[(String, [u8; 32])],
    proof: Option<ProofTimes>,
    dates: RecordDates,
) -> Vec<String> {
    let regs: Vec<RegisteredDigest<'_>> = registered
        .iter()
        .map(|(file, sha256)| RegisteredDigest {
            file,
            sha256: *sha256,
        })
        .collect();
    record::validate(m, &regs, proof, dates)
        .iter()
        .map(|f| format!("manifest.toml: {f}"))
        .collect()
}

/// `forecast-rules-hash <rules.toml>`: the rules' `FMRUL1` digest, written to `rules.sha256` beside them. Rules
/// that do not parse or still carry an `OPEN` marker are refused.
pub fn rules_hash(rules: &Path) -> Res<String> {
    let bytes = read(rules)?;
    parse_rules_bytes(&bytes, &rules.display().to_string())?;
    let hex = record::rules_digest(&bytes).to_hex();
    write_digest(&rules.with_file_name("rules.sha256"), &hex)?;
    Ok(hex)
}

fn git(root: &Path, args: &[&str]) -> Res<(bool, String)> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| vec![format!("cannot run git: {e}")])?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}

/// `forecast-rules-order`: the commit that added the FY2026 rules is a strict ancestor of every commit touching
/// the forecasts, so the baselines and the scoring rule were committed before any forecast (BD-334.S2). A shallow
/// clone is refused: its history cannot show the order.
pub fn rules_order(root: &Path) -> Res<String> {
    let (ok, shallow) = git(root, &["rev-parse", "--is-shallow-repository"])?;
    if !ok {
        return fail(format!("{} is not a git repository", root.display()));
    }
    if shallow == "true" {
        return fail(
            "shallow clone: the history that orders the rules and the forecasts is missing (fetch-depth: 0)"
                .to_string(),
        );
    }
    let (_, added) = git(
        root,
        &["log", "--format=%H", "--diff-filter=A", "--", FY2026_RULES],
    )?;
    let Some(rules_commit) = added.lines().last() else {
        return fail(format!("no commit adds {FY2026_RULES}"));
    };
    let (_, touched) = git(root, &["log", "--format=%H", "--", FY2026_FORECASTS])?;
    let mut errors = Vec::new();
    for c in touched.lines() {
        let (after, _) = git(root, &["merge-base", "--is-ancestor", rules_commit, c])?;
        if c == rules_commit || !after {
            errors.push(format!(
                "commit {c} touches {FY2026_FORECASTS} but does not follow the rules commit {rules_commit}"
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(format!(
        "{FY2026_RULES} added in {}; {} later commit(s) touch {FY2026_FORECASTS}",
        rules_commit.get(..7).unwrap_or(rules_commit),
        touched.lines().count()
    ))
}

/// `forecast-record-hash <dir>`: the record's `FMREC1` digest, written to `record.sha256`.
pub fn record_hash(root: &Path, dir: &Path) -> Res<String> {
    let loaded = load(root, dir)?;
    write_digest(&dir.join("record.sha256"), &loaded.digest)?;
    Ok(loaded.digest)
}

fn openssl(args: &[&str]) -> Res<String> {
    let out = Command::new("openssl")
        .args(args)
        .output()
        .map_err(|e| vec![format!("cannot run openssl: {e}")])?;
    if !out.status.success() {
        return fail(format!(
            "openssl {} failed: {} {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Seconds of one `Accuracy:` part: `0x01 seconds`, `500 millis`, `unspecified micros`.
fn accuracy_part(part: &str, unit: &str, per_second: u64) -> Result<u64, String> {
    let Some(value) = part.trim().strip_suffix(unit).map(str::trim) else {
        return Err(format!("accuracy part {part:?} is not in {unit}"));
    };
    if value == "unspecified" {
        return Ok(0);
    }
    let n = match value.strip_prefix("0x") {
        Some(h) => u64::from_str_radix(h, 16),
        None => value.parse(),
    }
    .map_err(|_| format!("accuracy value {value:?}"))?;
    Ok(n.div_ceil(per_second))
}

/// The proof interval from `openssl ts -reply -text`: `Time stamp: Oct 30 14:04:42 2026 GMT` (a fraction of a
/// second widens the upper bound by one second) widened both ways by `Accuracy:`, each part rounded up to whole
/// seconds; an unspecified part adds nothing.
pub fn proof_times_from_text(text: &str) -> Result<ProofTimes, String> {
    let line = |prefix: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(prefix).map(str::trim))
            .ok_or(format!("no {prefix:?} line"))
    };
    let stamp: Vec<&str> = line("Time stamp:")?.split_whitespace().collect();
    let [month, day, time, year, "GMT"] = stamp[..] else {
        return Err(format!("unreadable time stamp {stamp:?}"));
    };
    let m = MONTHS
        .iter()
        .position(|&x| x == month)
        .ok_or(format!("month {month:?}"))?;
    let (whole, fraction) = time.split_once('.').unwrap_or((time, ""));
    let iso = format!("{year}-{:02}-{:0>2}T{whole}Z", m + 1, day);
    let t = parse_utc(&iso).ok_or(format!("time stamp {iso:?}"))?;
    let parts: Vec<&str> = line("Accuracy:")?.split(',').collect();
    let accuracy = if parts == ["unspecified"] {
        0
    } else {
        let [s, ms, us] = parts[..] else {
            return Err(format!("unreadable accuracy {parts:?}"));
        };
        accuracy_part(s, "seconds", 1)?
            + accuracy_part(ms, "millis", 1_000)?
            + accuracy_part(us, "micros", 1_000_000)?
    };
    let accuracy = i64::try_from(accuracy).map_err(|_| "accuracy overflows".to_string())?;
    let partial = i64::from(fraction.bytes().any(|b| b != b'0'));
    Ok(ProofTimes {
        lower_utc: t - accuracy,
        upper_utc: t + partial + accuracy,
    })
}

/// `forecast-record-verify <dir>`: both digests recomputed, the RFC 3161 reply checked and its interval
/// validated against the manifest. Returns one line per check.
pub fn record_verify(root: &Path, dir: &Path) -> Res<Vec<String>> {
    let loaded = load(root, dir)?;
    let mut lines = Vec::new();
    let stored = fs::read_to_string(dir.join("record.sha256"))
        .map_err(|e| vec![format!("cannot read record.sha256: {e}")])?;
    if stored.trim_end() != loaded.digest {
        return fail(format!(
            "record.sha256 holds {}, but manifest.toml and forecasts.csv hash to {}",
            stored.trim_end(),
            loaded.digest
        ));
    }
    lines.push(format!("record digest {}", loaded.digest));
    lines.push(format!("rules digest {}", loaded.rules_digest));
    let tsr = dir.join("record.tsr");
    let chain = dir.join("tsa_chain.pem");
    if !tsr.exists() {
        return fail("no record.tsr beside the record: a time comes only from a proof".to_string());
    }
    let (tsr_s, chain_s) = (tsr.to_string_lossy(), chain.to_string_lossy());
    let out = openssl(&[
        "ts",
        "-verify",
        "-digest",
        &loaded.digest,
        "-in",
        &tsr_s,
        "-CAfile",
        &chain_s,
    ])?;
    if !out.contains("Verification: OK") {
        return fail(format!(
            "openssl ts -verify did not report OK: {}",
            out.trim()
        ));
    }
    lines.push("record.tsr verifies against tsa_chain.pem".to_string());
    let text = openssl(&["ts", "-reply", "-in", &tsr_s, "-text"])?;
    let proof = proof_times_from_text(&text).map_err(|e| vec![format!("record.tsr: {e}")])?;
    lines.push(format!(
        "proof interval [{}, {}], label {}",
        format_utc(proof.lower_utc),
        format_utc(proof.upper_utc),
        loaded.manifest.label.as_str()
    ));
    let found = findings(
        &loaded.manifest,
        &loaded.registered,
        Some(proof),
        loaded.dates,
    );
    if !found.is_empty() {
        return Err(found);
    }
    if dir.join("record.ots").exists() {
        lines.push(
            "record.ots present but not checked here: run `ots verify record.ots`".to_string(),
        );
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = "Status info:\nStatus: Granted.\n\nTST info:\nVersion: 1\nSerial number: 0x02\n\
                         Time stamp: Oct 30 14:04:42 2026 GMT\n\
                         Accuracy: 0x01 seconds, unspecified millis, unspecified micros\nOrdering: no\n";

    fn times(text: &str) -> (String, String) {
        let p = proof_times_from_text(text).expect("times");
        (format_utc(p.lower_utc), format_utc(p.upper_utc))
    }

    // The reply text is the shape `openssl ts -reply -text` (OpenSSL 4.0.3) printed for a local test TSA.
    #[test]
    fn test_proof_times_from_openssl_text() {
        assert_eq!(
            times(REPLY),
            (
                "2026-10-30T14:04:41Z".to_string(),
                "2026-10-30T14:04:43Z".to_string()
            )
        );
        let padded = REPLY
            .replace("Oct 30 14:04:42", "Nov  9 10:59:59.250")
            .replace(
                "0x01 seconds, unspecified millis",
                "unspecified seconds, 0x01F4 millis",
            );
        assert_eq!(
            times(&padded),
            (
                "2026-11-09T10:59:58Z".to_string(),
                "2026-11-09T11:00:01Z".to_string()
            )
        );
        let unspecified = REPLY.replace(
            "0x01 seconds, unspecified millis, unspecified micros",
            "unspecified",
        );
        assert_eq!(
            times(&unspecified),
            (
                "2026-10-30T14:04:42Z".to_string(),
                "2026-10-30T14:04:42Z".to_string()
            )
        );
        for bad in [
            REPLY.replace("GMT", "UTC"),
            REPLY.replace("Oct", "Okt"),
            REPLY.replace("Accuracy:", "Precision:"),
        ] {
            assert!(proof_times_from_text(&bad).is_err(), "{bad}");
        }
    }

    fn workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn committed_rules() -> (Vec<u8>, RecordRules) {
        let bytes = fs::read(workspace_root().join(FY2026_RULES)).expect("committed rules");
        let rules = parse_rules_bytes(&bytes, "rules.toml").expect("the committed rules parse");
        (bytes, rules)
    }

    #[test]
    fn test_committed_rules_parse_and_digest() {
        let (bytes, rules) = committed_rules();
        assert_eq!(rules.record_id, "FM-FY2026-v1");
        assert_eq!(rules.issuer, "barrick");
        assert_eq!(rules.dates, record::FY2026_DATES);
        assert_eq!((rules.alpha.num(), rules.alpha.den()), (1, 5));
        assert_eq!(rules.estimator, record::Estimator::InverseEcdf);
        let ids: Vec<&str> = rules.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["production", "unit_cost"]);
        for t in &ids {
            let mut baselines: Vec<&str> = rules
                .baselines
                .iter()
                .filter(|b| b.target == *t)
                .map(|b| b.id.as_str())
                .collect();
            baselines.sort_unstable();
            assert_eq!(
                baselines,
                ["issuer_guidance", "run_rate_persistence"],
                "{t}"
            );
        }
        let stored = fs::read_to_string(workspace_root().join("forecasts/fy2026/rules.sha256"))
            .expect("rules.sha256");
        assert_eq!(stored.trim_end(), record::rules_digest(&bytes).to_hex());
    }

    #[test]
    fn test_rules_have_no_open_markers() {
        let (bytes, _) = committed_rules();
        let text = String::from_utf8(bytes).expect("UTF-8");
        for (n, line) in text.lines().enumerate() {
            assert!(!record::has_open_marker(line), "line {}", n + 1);
        }
        // Planted: a marker left in a comment is refused too, before any digest is written.
        let planted = format!("# OPEN: the estimator\n{text}");
        assert!(parse_rules_bytes(planted.as_bytes(), "rules.toml").is_err());
    }

    #[test]
    fn test_rules_site_universe_is_named_list() {
        let (_, rules) = committed_rules();
        let names: Vec<&str> = rules.sites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Carlin",
                "Cortez",
                "Turquoise Ridge",
                "Phoenix",
                "Pueblo Viejo",
                "Hemlo",
                "Veladero",
                "Porgera",
                "Loulo-Gounkoto",
                "Kibali",
                "Tongon",
                "North Mara",
                "Bulyanhulu"
            ]
        );
        let not_published: Vec<&str> = rules
            .sites
            .iter()
            .filter(|s| s.status == record::SiteStatus::NotPublished)
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(not_published, ["Hemlo", "Tongon"]);
        assert!(
            rules
                .sites
                .iter()
                .all(|s| s.locator.starts_with("Q2_2026:QTD!A"))
        );
    }

    #[test]
    fn test_rules_commit_precedes_forecasts() {
        let res = rules_order(&workspace_root());
        assert!(res.is_ok(), "{res:?}");
    }
}
