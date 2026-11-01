//! The public forecast record's IO (BD-334.S1). `fmine_calib::record` is pure, so reading the files, writing the
//! digests and reading a time proof's dates happen here:
//! - `forecast-rules-hash <rules.toml>` writes `rules.sha256` beside the rules (their `FMRUL1` digest);
//! - `forecast-record-hash <dir>` validates `manifest.toml` against the source registries and the `rules.toml`
//!   beside it, then writes `record.sha256` (the `FMREC1` digest of `manifest.toml` and `forecasts.csv`);
//! - `forecast-record-verify <dir>` recomputes both digests, checks the RFC 3161 reply `record.tsr` against the
//!   pinned checked-in public authority roots with `openssl ts -verify`, reads the signed time and accuracy,
//!   requires an earlier authenticated rules proof, and validates the manifest against that interval;
//!   `tsa_chain.pem`, when supplied, contributes only untrusted intermediate certificates;
//! - `forecast-rules-order` checks in git history that the FY2026 rules were committed before any forecast
//!   (BD-334.S2).
//!
//! A time comes only from a verified proof, never from a clock. A digest file is replaced only while no proof
//! stands beside it: once stamped, a changed file is refused rather than re-hashed. The rules must parse and carry
//! no `OPEN` marker before they are hashed, and the record is judged against the dates its rules state.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use std::collections::BTreeMap;

mod scoring;
pub use scoring::record_score;

use fmine_calib::record::{
    self, BaselineKind, ProofTimes, Quarter, Rat, RecordDates, RecordManifest, RecordRules,
    RegisteredDigest, SiteStatus, format_utc, parse_hex32, parse_utc,
};
use fmine_public::mine_stats::{
    LabelVocabulary, MineStatsCell, MineStatsValue, View, read_mine_stats,
};

pub mod guidance;

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

/// Writes `hex` to `path` (`<stem>.sha256`). A file holding a different digest is replaced only while no proof
/// (`<stem>.tsr` or `<stem>.ots`) stands beside it: once stamped, a changed file is refused, never re-hashed.
fn write_digest(path: &Path, hex: &str) -> Res<()> {
    match fs::read_to_string(path) {
        Ok(old) if old.trim_end() == hex => return Ok(()),
        Ok(old) => {
            let stamp = ["tsr", "ots"]
                .iter()
                .map(|x| path.with_extension(x))
                .find(|p| p.exists());
            if let Some(stamp) = stamp {
                return fail(format!(
                    "{} already holds {}, not {hex}, and {} stamps it: a stamped file changed; restore it or \
                     start a new version",
                    path.display(),
                    old.trim_end(),
                    stamp.display()
                ));
            }
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
        let rows = reg
            .sources
            .into_iter()
            .map(|source| (source.file, source.sha256))
            .chain(
                reg.documents
                    .into_iter()
                    .map(|document| (document.file, document.sha256)),
            );
        for (file, sha256) in rows {
            let Some(digest) = parse_hex32(&sha256) else {
                return fail(format!("{}: {file} has no 64-digit sha256", p.display()));
            };
            out.push((file, digest));
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
    rules: RecordRules,
    forecasts: Vec<record::ForecastRow>,
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
    let forecasts = record::parse_forecasts(&csv, &rules).map_err(|e| vec![e.to_string()])?;
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
        rules,
        forecasts,
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

/// An exact value as `n` or `n/d`.
fn exact_text(x: Rat) -> String {
    if x.den() == 1 {
        x.num().to_string()
    } else {
        format!("{}/{}", x.num(), x.den())
    }
}

/// The value a persistence series takes for `site` in quarter `q`: the summary field's three-month value as first
/// published, i.e. from the quarter's own release (its current-year block, period `q`). `None` when it is not a
/// positive printed number: a placeholder, a blank, a zero or no row. Two candidate cells refuse. Messages carry
/// locators, never values (RISK-31).
fn series_value(
    values: &[MineStatsValue],
    q: Quarter,
    site: &str,
    field: &str,
) -> Result<Option<Rat>, String> {
    let period = q.to_string();
    let mut hits = values.iter().filter(|v| {
        v.view == View::Summary
            && v.months == 3
            && v.period == period
            && v.site == site
            && v.label == field
    });
    let first = hits.next();
    if let (Some(a), Some(b)) = (first, hits.next()) {
        return Err(format!(
            "{period} {site} {field}: two cells, {} and {}",
            a.locator, b.locator
        ));
    }
    match first {
        Some(MineStatsValue {
            value: MineStatsCell::Number { raw_text, .. },
            locator,
            ..
        }) => {
            let x = Rat::from_decimal_str(raw_text)
                .map_err(|_| format!("{locator}: the stored number is not an exact decimal"))?;
            Ok((x.signum() > 0).then_some(x))
        }
        _ => Ok(None),
    }
}

/// One live site's persistence interval: `point × [q_lo, q_hi]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiteInterval {
    pub point: Rat,
    pub lower: Rat,
    pub upper: Rat,
}

/// A run-rate persistence baseline of the FY2026 rules, computed from the registered Mine Stats workbooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistenceBaseline {
    pub target: String,
    pub pool_from: Quarter,
    pub pool_through: Quarter,
    pub point: Quarter,
    /// Ratios in the pool, and consecutive pairs left out of it (either value not a positive printed number).
    pub ratios: usize,
    pub left_out: usize,
    /// The ratios at α/2 and 1 − α/2.
    pub q_lo: Rat,
    pub q_hi: Rat,
    /// Per live site, its interval, or `None` where the point is not a positive printed number. `None` as a whole
    /// until the point quarter's workbook is registered.
    pub intervals: Option<Vec<(String, Option<SiteInterval>)>>,
}

/// Every `run_rate_persistence` baseline of `forecasts/fy2026/rules.toml`, from the issuer's registered workbooks
/// read label-anchored under the registered vocabulary. Refuses a workbook with any refused cell, a pool quarter
/// that is not registered and an ambiguous cell; the pool, the point quarter, α and the estimator come from the
/// rules alone.
pub fn persistence_baselines(root: &Path) -> Res<Vec<PersistenceBaseline>> {
    let rules = parse_rules_bytes(&read(&root.join(FY2026_RULES))?, FY2026_RULES)?;
    let profile =
        fmine_public::load_issuer_profile(root, &rules.issuer).map_err(|e| vec![e.to_string()])?;
    let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
    let sources =
        fmine_public::load_registered_sources(&profile, root).map_err(|e| vec![e.to_string()])?;
    let mut reads: BTreeMap<Quarter, Vec<MineStatsValue>> = BTreeMap::new();
    for src in sources
        .iter()
        .filter(|s| s.kind == fmine_public::sources::SourceKind::MineStats)
    {
        let q = Quarter::parse(&src.quarter)
            .ok_or_else(|| vec![format!("{}: quarter {:?}", src.file_name, src.quarter)])?;
        let read = read_mine_stats(&src.workbook, &src.quarter, Some(&vocabulary))
            .map_err(|e| vec![format!("{}: {e}", src.file_name)])?;
        if !read.refusals.is_empty() {
            return fail(format!(
                "{}: {} refused cells; a baseline is computed only from a clean read",
                src.file_name,
                read.refusals.len()
            ));
        }
        reads.insert(q, read.values);
    }
    let live: Vec<&str> = rules
        .sites
        .iter()
        .filter(|s| s.status == SiteStatus::Live)
        .map(|s| s.name.as_str())
        .collect();
    let mut out = Vec::new();
    for b in &rules.baselines {
        let BaselineKind::Persistence {
            pool_from,
            pool_through,
            point,
        } = b.kind
        else {
            continue;
        };
        let Some(target) = rules.targets.iter().find(|t| t.id == b.target) else {
            return fail(format!("baseline target {:?} is not declared", b.target));
        };
        let mut series = Vec::with_capacity(live.len());
        for site in &live {
            let mut s = Vec::new();
            for q in pool_from.through(pool_through) {
                let values = reads.get(&q).ok_or_else(|| {
                    vec![format!(
                        "{q} is in the pool, but no Mine Stats workbook is registered for it"
                    )]
                })?;
                s.push(series_value(values, q, site, &target.field).map_err(|e| vec![e])?);
            }
            series.push(s);
        }
        let (ratios, left_out) =
            record::pooled_ratios(&series).map_err(|e| vec![format!("{}: {e}", b.target)])?;
        let (q_lo, q_hi) = record::ratio_quantiles(&ratios, rules.alpha, rules.estimator)
            .map_err(|e| vec![format!("{}: {e}", b.target)])?;
        let intervals = match reads.get(&point) {
            None => None,
            Some(values) => {
                let mut rows = Vec::with_capacity(live.len());
                for site in &live {
                    let iv = match series_value(values, point, site, &target.field)
                        .map_err(|e| vec![e])?
                    {
                        None => None,
                        Some(point) => {
                            let (lower, upper) = record::persistence_interval(
                                &ratios,
                                point,
                                rules.alpha,
                                rules.estimator,
                            )
                            .map_err(|e| vec![format!("{site} {}: {e}", b.target)])?;
                            Some(SiteInterval {
                                point,
                                lower,
                                upper,
                            })
                        }
                    };
                    rows.push(((*site).to_string(), iv));
                }
                Some(rows)
            }
        };
        out.push(PersistenceBaseline {
            target: b.target.clone(),
            pool_from,
            pool_through,
            point,
            ratios: ratios.len(),
            left_out,
            q_lo,
            q_hi,
            intervals,
        });
    }
    Ok(out)
}

/// `forecast-baseline`: one JSON line per persistence baseline (the pool's size, the pairs left out and the two
/// ratio quantiles), then, once the point quarter is registered, one `site,target,point,lower,upper` line per live
/// site, exact (`n/d`). The site lines hold issuer-derived values: the command needs the corpus, so it never runs
/// in CI (RISK-31).
pub fn baseline_lines(root: &Path) -> Res<Vec<String>> {
    let mut lines = Vec::new();
    for b in persistence_baselines(root)? {
        lines.push(format!(
            "{{\"baseline\":\"run_rate_persistence\",\"target\":\"{}\",\"pool\":\"{}..{}\",\"ratios\":{},\
             \"left_out\":{},\"q_lo\":\"{}\",\"q_hi\":\"{}\",\"point_quarter\":\"{}\",\"point_registered\":{}}}",
            b.target,
            b.pool_from,
            b.pool_through,
            b.ratios,
            b.left_out,
            exact_text(b.q_lo),
            exact_text(b.q_hi),
            b.point,
            b.intervals.is_some()
        ));
        for (site, iv) in b.intervals.iter().flatten() {
            lines.push(match iv {
                Some(iv) => format!(
                    "{site},{},{},{},{}",
                    b.target,
                    exact_text(iv.point),
                    exact_text(iv.lower),
                    exact_text(iv.upper)
                ),
                None => format!("{site},{},NOT_PUBLISHED", b.target),
            });
        }
    }
    Ok(lines)
}

/// Both frozen baseline rules at an explicit evidence cut. The legacy no-argument command remains the
/// persistence-only projection. Workbooks carry a verified retrieval bound, not invented publication dates.
pub fn baseline_lines_as_of(root: &Path, as_of: &str) -> Res<Vec<String>> {
    let guidance = guidance::run(root, as_of)?;
    let mut lines = guidance.json_lines();
    lines.extend(baseline_lines(root)?);
    Ok(lines)
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
        [
            accuracy_part(s, "seconds", 1)?,
            accuracy_part(ms, "millis", 1_000)?,
            accuracy_part(us, "micros", 1_000_000)?,
        ]
        .into_iter()
        .try_fold(0_u64, u64::checked_add)
        .ok_or("accuracy overflows")?
    };
    let accuracy = i64::try_from(accuracy).map_err(|_| "accuracy overflows".to_string())?;
    let partial = i64::from(fraction.bytes().any(|b| b != b'0'));
    // An accuracy wide enough to leave the representable times widens nothing: it refuses.
    match (
        t.checked_sub(accuracy),
        t.checked_add(partial).and_then(|u| u.checked_add(accuracy)),
    ) {
        (Some(lower_utc), Some(upper_utc)) => Ok(ProofTimes {
            lower_utc,
            upper_utc,
        }),
        _ => Err(format!(
            "accuracy of {accuracy} s overflows the proof interval"
        )),
    }
}

/// Public timestamp authorities accepted by the record protocol. The SHA-256 pins the complete checked-in
/// PEM bytes, not a caller's replacement certificate or a file named tsa_chain.pem.
const TSA_ROOTS: [(&str, &str); 2] = [
    (
        "forecasts/tsa/digicert-trusted-root-g4.pem",
        "ce7d6b44f5d510391be98c8d76b18709400a30cd87659bfebe1c6f97ff5181ee",
    ),
    (
        "forecasts/tsa/freetsa-root.pem",
        "2151b61137ffa86bf664691ba67e7da0b19f98c758e3d228d5d8ebf27e044438",
    ),
];

fn trusted_timestamp(
    root: &Path,
    dir: &Path,
    file: &str,
    digest: &str,
) -> Res<(ProofTimes, String)> {
    let path = dir.join(file);
    let stamp = read(&path)?;
    let path_text = path.to_string_lossy();
    // Parsing does not authenticate these times. Authentication below checks the signature and certificate
    // at the token's own genTime, so an authority certificate expiring later does not erase an old record.
    let text = openssl(&["ts", "-reply", "-in", &path_text, "-text"])?;
    let proof = proof_times_from_text(&text).map_err(|e| vec![format!("{file}: {e}")])?;
    let midpoint = i128::from(proof.lower_utc)
        + (i128::from(proof.upper_utc) - i128::from(proof.lower_utc)) / 2;
    let attime = midpoint.to_string();
    let intermediates = dir.join("tsa_chain.pem");
    let intermediates_text = intermediates.to_string_lossy();
    let mut failures = Vec::new();
    for (relative, expected) in TSA_ROOTS {
        let ca = root.join(relative);
        let bytes = match read(&ca) {
            Ok(bytes) => bytes,
            Err(errors) => {
                failures.extend(errors);
                continue;
            }
        };
        if fmine_core::hash::sha256(&bytes).to_hex() != expected {
            failures.push(format!(
                "{relative}: trusted-root bytes differ from the compiled pin"
            ));
            continue;
        }
        let ca_text = ca.to_string_lossy();
        // OpenSSL ts (3.x and 4.x apps/ts.c::create_cert_store) builds an empty trust store and adds
        // only explicitly supplied CAfile/CApath/CAstore. Unlike openssl verify, ts has no
        // -no-CApath/-no-CAstore options. Supplying only CAfile therefore excludes ambient roots.
        let mut args = vec![
            "ts", "-verify", "-digest", digest, "-in", &path_text, "-CAfile", &ca_text, "-attime",
            &attime,
        ];
        if intermediates.exists() {
            args.extend(["-untrusted", &intermediates_text]);
        }
        match openssl(&args) {
            Ok(output) if output.contains("Verification: OK") => {
                if read(&path)? != stamp || read(&ca)? != bytes {
                    return fail("timestamp or trust root changed during verification".into());
                }
                return Ok((proof, relative.into()));
            }
            Ok(_) => failures.push(format!(
                "{file}: verification did not report OK with {relative}"
            )),
            Err(errors) => failures.extend(errors),
        }
    }
    failures.insert(
        0,
        format!("{file}: no trusted RFC 3161 proof; a record-supplied root is not an authority"),
    );
    Err(failures)
}

/// Verifies the exact already-loaded manifest, forecasts and rules. Both verify and score use this path;
/// scoring never rereads mutable record payloads after establishing their frozen digest.
fn verify_loaded(root: &Path, dir: &Path, loaded: &Loaded) -> Res<(Vec<String>, ProofTimes)> {
    let stored = fs::read_to_string(dir.join("record.sha256"))
        .map_err(|e| vec![format!("cannot read record.sha256: {e}")])?;
    if stored.trim_end() != loaded.digest {
        return fail(format!(
            "record.sha256 holds {}, but manifest.toml and forecasts.csv hash to {}",
            stored.trim_end(),
            loaded.digest
        ));
    }
    let stored_rules = fs::read_to_string(dir.join("rules.sha256"))
        .map_err(|e| vec![format!("cannot read rules.sha256: {e}")])?;
    if stored_rules.trim_end() != loaded.rules_digest {
        return fail("rules.sha256 differs from the rules bound by the frozen manifest".into());
    }
    let (proof, authority) = trusted_timestamp(root, dir, "record.tsr", &loaded.digest)?;
    let found = findings(
        &loaded.manifest,
        &loaded.registered,
        Some(proof),
        loaded.dates,
    );
    if !found.is_empty() {
        return Err(found);
    }
    let mut prior_rules = Vec::new();
    let mut rule_failures = Vec::new();
    for file in ["rules.tsr", "rules.freetsa.tsr"] {
        if dir.join(file).exists() {
            match trusted_timestamp(root, dir, file, &loaded.rules_digest) {
                Ok((times, authority)) => {
                    prior_rules.push((times.upper_utc, file, times, authority))
                }
                Err(errors) => rule_failures.extend(errors),
            }
        }
    }
    prior_rules.sort_by_key(|p| p.0);
    let Some((_, rules_file, rules_proof, rules_authority)) = prior_rules.first() else {
        rule_failures.insert(0, "no trusted timestamp for the frozen rules".into());
        return Err(rule_failures);
    };
    if rules_proof.upper_utc >= proof.lower_utc {
        return fail("the rules proof must strictly precede the record proof, including both accuracy bounds".into());
    }
    let mut lines = vec![
        format!("record digest {}", loaded.digest),
        format!("rules digest {}", loaded.rules_digest),
        format!("record.tsr verifies against pinned {authority}"),
        format!("{rules_file} verifies against pinned {rules_authority} before record.tsr"),
        format!(
            "proof interval [{}, {}], label {}",
            format_utc(proof.lower_utc),
            format_utc(proof.upper_utc),
            loaded.manifest.label.as_str()
        ),
    ];
    if dir.join("record.ots").exists() {
        lines.push("record.ots present but not checked here: run ots verify record.ots".into());
    }
    Ok((lines, proof))
}

/// forecast-record-verify: frozen digests, independently signed timestamps under pinned public
/// authorities, strict rules-before-record ordering, and the manifest's information cut and deadline.
pub fn record_verify(root: &Path, dir: &Path) -> Res<Vec<String>> {
    let loaded = load(root, dir)?;
    verify_loaded(root, dir, &loaded).map(|(lines, _)| lines)
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
            REPLY.replace("0x01 seconds", "0x7fffffffffffffff seconds"),
            REPLY
                .replace("2026 GMT", "1960 GMT")
                .replace("0x01 seconds", "0x7fffffffffffffff seconds"),
            REPLY.replace(
                "0x01 seconds, unspecified millis",
                "0xffffffffffffffff seconds, 0xffffffffffffffff millis",
            ),
        ] {
            assert!(proof_times_from_text(&bad).is_err(), "{bad}");
        }
    }

    fn workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn cell(
        view: View,
        months: u8,
        period: &str,
        site: &str,
        label: &str,
        value: MineStatsCell,
    ) -> MineStatsValue {
        MineStatsValue {
            locator: format!("{period}:{site}:{label}"),
            sheet: "QTD".to_string(),
            cell: "B5".to_string(),
            view,
            months,
            period: period.to_string(),
            period_label: String::new(),
            site_raw: site.to_string(),
            site: site.to_string(),
            printed_share: None,
            label: label.to_string(),
            label_as_printed: label.to_string(),
            footnote: None,
            section: None,
            value,
        }
    }

    fn number(text: &str) -> MineStatsCell {
        MineStatsCell::Number {
            raw_text: text.to_string(),
            value: text.parse().expect("f64"),
        }
    }

    /// The series value is the quarter's own three-month summary cell: not a comparative year, a longer period,
    /// a Minestats row or another field. Placeholders, zero and no row are `None`; two candidates refuse.
    #[test]
    fn test_series_value_takes_the_first_published_quarter() {
        let f = "au_prod_100_koz";
        let q = Quarter::parse("2026Q2").expect("quarter");
        let values = vec![
            cell(View::Summary, 3, "2025Q2", "Carlin", f, number("300")),
            cell(View::Summary, 6, "2026H1", "Carlin", f, number("700")),
            cell(View::Minestats, 3, "2026Q2", "Carlin", f, number("1")),
            cell(
                View::Summary,
                3,
                "2026Q2",
                "Carlin",
                "au_prod_attr_koz",
                number("250"),
            ),
            cell(View::Summary, 3, "2026Q2", "Carlin", f, number("4.105E2")),
            cell(
                View::Summary,
                3,
                "2026Q2",
                "Hemlo",
                f,
                MineStatsCell::Nil {
                    token: "—".into()
                },
            ),
            cell(View::Summary, 3, "2026Q2", "Tongon", f, number("0")),
            cell(View::Summary, 3, "2026Q2", "Kibali", f, number("1")),
            cell(View::Summary, 3, "2026Q2", "Kibali", f, number("2")),
        ];
        let at = |site| series_value(&values, q, site, f);
        assert_eq!(at("Carlin"), Ok(Some(Rat::new(821, 2).expect("rat"))));
        assert_eq!(at("Hemlo"), Ok(None));
        assert_eq!(at("Tongon"), Ok(None));
        assert_eq!(at("Porgera"), Ok(None));
        assert!(at("Kibali").expect_err("two cells").contains("two cells"));
        assert_eq!(exact_text(Rat::new(821, 2).expect("rat")), "821/2");
        assert_eq!(exact_text(Rat::from_integer(7)), "7");
    }

    /// The FY2026 persistence baselines on the ten registered workbooks (local corpus): both targets pool the 11
    /// live sites' 9 consecutive pairs, 2024Q1-2026Q2, and with the 2026Q3 workbook not yet registered no site gets
    /// an interval. Prints counts only (RISK-31).
    #[test]
    #[ignore = "needs the registered Mine Stats workbooks under corpus_of_input_docs/"]
    fn test_fy2026_persistence_baselines_on_registered_files() {
        let baselines = persistence_baselines(&workspace_root()).expect("baselines");
        assert_eq!(
            baselines
                .iter()
                .map(|b| b.target.as_str())
                .collect::<Vec<_>>(),
            ["production", "unit_cost"]
        );
        let one = Rat::from_integer(1);
        for b in &baselines {
            eprintln!(
                "{}: {} ratios, {} pairs left out",
                b.target, b.ratios, b.left_out
            );
            assert_eq!(b.ratios + b.left_out, 11 * 9, "{}", b.target);
            assert!(
                b.ratios >= 50,
                "{}: most pairs are printed numbers",
                b.target
            );
            assert_eq!(b.q_lo.checked_cmp(one), Ok(std::cmp::Ordering::Less));
            assert_eq!(b.q_hi.checked_cmp(one), Ok(std::cmp::Ordering::Greater));
            assert_eq!(b.intervals, None, "2026Q3 is not registered yet");
        }
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

    // Ignored by default: it needs the repository's history, which a source copy (an rch worker, a package)
    // lacks. CI runs it with --ignored on a full-history checkout.
    #[test]
    #[ignore = "needs the git history; CI runs it with --ignored"]
    fn test_rules_commit_precedes_forecasts() {
        let res = rules_order(&workspace_root());
        assert!(res.is_ok(), "{res:?}");
    }

    /// A throwaway repository, removed on drop so that a failing test leaves nothing behind. On an rch worker the
    /// temp dir sits inside the project, and a host that declares live checkouts refuses every later sync of this
    /// project once one `.git` is left there (`RCH_SYNC_LIVE_CHECKOUT_REFUSED`).
    struct Repo(std::path::PathBuf);

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    impl std::ops::Deref for Repo {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    /// A repository under the system temp dir with one commit per entry, each writing the named files.
    fn repo(name: &str, commits: &[&[&str]]) -> Repo {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = Repo(std::env::temp_dir().join(format!(
            "fmine-rules-order-{}-{nanos}-{name}",
            std::process::id()
        )));
        fs::create_dir_all(dir.join("forecasts/fy2026")).expect("repo dir");
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&*dir)
                .args([
                    "-c",
                    "user.name=test",
                    "-c",
                    "user.email=test@example.invalid",
                ])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .expect("git runs");
            assert!(out.status.success(), "git {args:?}: {out:?}");
        };
        git(&["init", "-q"]);
        for (k, files) in commits.iter().enumerate() {
            for f in *files {
                fs::write(dir.join(f), format!("{k}\n")).expect("write");
            }
            git(&["add", "."]);
            git(&["commit", "-q", "-m", &format!("commit {k}")]);
        }
        dir
    }

    #[test]
    fn test_digest_frozen_only_once_stamped() {
        let dir = repo("digest", &[]);
        let path = dir.join("rules.sha256");
        let (a, b) = ("a".repeat(64), "b".repeat(64));
        assert!(write_digest(&path, &a).is_ok());
        // Not yet stamped: a revised file is re-hashed.
        assert!(write_digest(&path, &b).is_ok());
        assert_eq!(fs::read_to_string(&path).expect("digest"), format!("{b}\n"));
        for proof in ["rules.tsr", "rules.ots"] {
            let stamped = repo(proof, &[]);
            let p = stamped.join("rules.sha256");
            assert!(write_digest(&p, &a).is_ok());
            fs::write(stamped.join(proof), "proof").expect("proof");
            assert!(write_digest(&p, &a).is_ok(), "the same digest stands");
            let err = write_digest(&p, &b).expect_err("stamped");
            assert!(err[0].contains("stamps it"), "{err:?}");
            assert_eq!(fs::read_to_string(&p).expect("digest"), format!("{a}\n"));
        }
    }

    #[test]
    fn test_rules_order_refuses_forecasts_not_after_rules() {
        let ordered = repo("ordered", &[&[FY2026_RULES], &[FY2026_FORECASTS]]);
        assert!(rules_order(&ordered).is_ok());
        for (name, commits) in [
            (
                "forecasts_first",
                &[&[FY2026_FORECASTS][..], &[FY2026_RULES][..]][..],
            ),
            ("same_commit", &[&[FY2026_RULES, FY2026_FORECASTS][..]][..]),
        ] {
            let err = rules_order(&repo(name, commits)).expect_err(name);
            assert!(
                err[0].contains("does not follow the rules commit"),
                "{name}: {err:?}"
            );
        }
        let no_rules = repo("no_rules", &[&[FY2026_FORECASTS]]);
        let err = rules_order(&no_rules).expect_err("no rules commit");
        assert!(err[0].contains("no commit adds"), "{err:?}");
        // A depth-1 clone of the ordered history: its one commit would seem to add both files at once.
        let shallow = Repo(ordered.with_extension("shallow"));
        let out = Command::new("git")
            .args(["clone", "-q", "--depth", "1"])
            .arg(format!("file://{}", ordered.display()))
            .arg(&*shallow)
            .output()
            .expect("git clone runs");
        assert!(out.status.success(), "{out:?}");
        let err = rules_order(&shallow).expect_err("shallow clone");
        assert!(err[0].contains("shallow clone"), "{err:?}");
    }
    /// Synthetic manifest, real registered metadata: PDFs must obey the same digest binding as workbooks.
    #[test]
    fn registered_document_hashes_are_bound_by_manifest_validation() {
        let root = workspace_root();
        let registry = fmine_registry::sources::load_sources_registry(
            &root.join("registries/sources/barrick.toml"),
        )
        .expect("registered metadata");
        let document = registry
            .documents
            .iter()
            .find(|document| document.file == "Barrick_Q4_2025_MD-A.pdf")
            .expect("registered PDF");
        let registered = registered_digests(&root).expect("all registered source digests");
        let digest = parse_hex32(&document.sha256).expect("registered digest");
        assert!(registered.contains(&(document.file.clone(), digest)));
        for workbook in &registry.sources {
            assert!(registered.contains(&(
                workbook.file.clone(),
                parse_hex32(&workbook.sha256).expect("workbook digest"),
            )));
        }
        let mut manifest = RecordManifest {
            record_id: "synthetic-registry-binding".into(),
            author_role: "public-lane-author".into(),
            information_cutoff_utc: parse_utc("2026-11-01T00:00:00Z").expect("cut"),
            label: record::Label::Forecast,
            contact: record::Contact::PreContact,
            attestation: record::ATTESTATION.into(),
            rules_sha256: [1; 32],
            inputs: vec![record::InputDoc {
                doc_id: document.file.clone(),
                url: document.url.clone(),
                publication_day: record::parse_date(&document.published_at)
                    .expect("publication date"),
                retrieved_utc: parse_utc("2026-10-31T12:00:00Z").expect("declared retrieval"),
                bytes: document.bytes,
                sha256: digest,
                line: 1,
            }],
        };
        assert!(findings(&manifest, &registered, None, record::FY2026_DATES).is_empty());
        manifest.inputs[0].sha256[0] ^= 1;
        let refused = findings(&manifest, &registered, None, record::FY2026_DATES);
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(refused[0].contains(&document.file));
    }

    #[test]
    fn committed_rules_verify_under_pinned_public_authorities_at_their_signed_time() {
        let root = workspace_root();
        let dir = root.join("forecasts/fy2026");
        let bytes = fs::read(dir.join("rules.toml")).unwrap();
        let digest = record::rules_digest(&bytes).to_hex();
        for (path, expected) in TSA_ROOTS {
            let bytes = fs::read(root.join(path)).unwrap();
            assert_eq!(fmine_core::hash::sha256(&bytes).to_hex(), expected);
        }
        let (proof, authority) = trusted_timestamp(&root, &dir, "rules.tsr", &digest).unwrap();
        assert!(authority.ends_with("digicert-trusted-root-g4.pem"));
        assert!(proof.upper_utc < record::FY2026_DATES.nowcast_from);
        assert!(trusted_timestamp(&root, &dir, "rules.tsr", &"00".repeat(32)).is_err());
    }
}
