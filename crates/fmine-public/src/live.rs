//! Live-dependency proof, file half (BD-338.S1): every registered file is load-bearing at the gate.
//!
//! For each registered file, a scratch workspace holds copies of the other files, and that file absent, written
//! as zero bytes, or replaced by a valid but foreign xlsx. The loader must refuse each case with `FMINE-REF-502`
//! (exit 8, a catalogued code), count "1 of n registered", and name that file, and no other, under the label
//! that applies: "missing" for an absent file, "digest mismatch" for a changed one. Only a refusal passes;
//! a loader that falls back to other bytes, or re-raises under an unregistered code, fails by name.

use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::{Refusal, lookup_refusal_spec};
use fmine_io::xlsx::build_minimal_xlsx;

use crate::LoadedSource;
use crate::sources::IssuerProfile;

/// What a check does to the one file it is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    FileDeleted,
    FileZeroBytes,
    FileForeign,
}

impl CheckKind {
    pub const ALL: [Self; 3] = [Self::FileDeleted, Self::FileZeroBytes, Self::FileForeign];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileDeleted => "file_deleted",
            Self::FileZeroBytes => "file_zero_bytes",
            Self::FileForeign => "file_foreign",
        }
    }

    /// The refusal label the file must be named under.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::FileDeleted => "missing",
            Self::FileZeroBytes | Self::FileForeign => "digest mismatch",
        }
    }

    fn not_refused(self) -> &'static str {
        match self {
            Self::FileDeleted => "file deletion did not refuse",
            Self::FileZeroBytes => "zero-byte file did not refuse",
            Self::FileForeign => "foreign file did not refuse",
        }
    }
}

/// One check: the loader's answer for one file and one kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCheck {
    pub kind: CheckKind,
    pub file: String,
    /// The refusal's code and exit, or `None` when the loader returned `Ok`.
    pub observed: Option<(&'static str, u8)>,
    pub names_only_this_file: bool,
    /// Why the check failed; `None` when it passed.
    pub failure: Option<String>,
}

impl LiveCheck {
    #[must_use]
    pub fn pass(&self) -> bool {
        self.failure.is_none()
    }

    /// One JSON log line.
    #[must_use]
    pub fn to_json(&self) -> String {
        let (code, exit) = self.observed.map_or_else(
            || ("null".to_string(), "null".to_string()),
            |(c, e)| (format!("\"{c}\""), e.to_string()),
        );
        let mut o = format!(
            "{{\"kind\":\"{}\",\"file\":\"{}\",\"expected_code\":\"FMINE-REF-502\",\"expected_exit\":8,\
             \"observed_code\":{code},\"observed_exit\":{exit},\"label\":\"{}\",\"names_only_this_file\":{},\
             \"pass\":{}",
            self.kind.as_str(),
            self.file,
            self.kind.label(),
            self.names_only_this_file,
            self.pass()
        );
        if let Some(f) = &self.failure {
            let _ = write!(o, ",\"failure\":\"{}\"", f.replace('"', "'"));
        }
        o.push('}');
        o
    }
}

/// The checks of one proof, keyed by the ledger anchor of the intact run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReport {
    pub anchor: String,
    pub checks: Vec<LiveCheck>,
}

impl LiveReport {
    /// One line per check, then `{"anchor","files","checks","pass"}`.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let mut o = String::new();
        let mut files: Vec<&str> = self.checks.iter().map(|c| c.file.as_str()).collect();
        files.dedup();
        for c in &self.checks {
            o.push_str(&c.to_json());
            o.push('\n');
        }
        let _ = writeln!(
            o,
            "{{\"anchor\":\"{}\",\"files\":{},\"checks\":{},\"pass\":{}}}",
            self.anchor,
            files.len(),
            self.checks.len(),
            self.checks.iter().filter(|c| c.pass()).count()
        );
        o
    }
}

/// Signature of the loader under proof: [`crate::load_registered_sources`], or a wrapper around it.
pub type Loader<'a> = &'a dyn Fn(&IssuerProfile, &Path) -> Result<Vec<LoadedSource>, Refusal>;

/// Runs the three file checks on every registered file of `profile`. The intact files are read from
/// `corpus_root`/`corpus_dir`; each case gets its own workspace under `scratch` (never the real corpus).
///
/// # Errors
/// An I/O error reading an intact file or writing a scratch workspace.
pub fn prove_files(
    profile: &IssuerProfile,
    corpus_root: &Path,
    scratch: &Path,
    load: Loader<'_>,
) -> std::io::Result<Vec<LiveCheck>> {
    let n = profile.sources.len();
    let intact = corpus_root.join(&profile.corpus_dir);
    let names: Vec<&str> = profile
        .sources
        .iter()
        .map(|s| s.file_name.as_str())
        .collect();
    let mut checks = Vec::with_capacity(n * CheckKind::ALL.len());
    for (i, file) in names.iter().enumerate() {
        for kind in CheckKind::ALL {
            let ws = scratch.join(format!("{i}-{}", kind.as_str()));
            let corpus = ws.join(&profile.corpus_dir);
            std::fs::create_dir_all(&corpus)?;
            for other in names.iter().filter(|o| *o != file) {
                std::fs::copy(intact.join(other), corpus.join(other))?;
            }
            match kind {
                CheckKind::FileDeleted => {}
                CheckKind::FileZeroBytes => std::fs::write(corpus.join(file), [])?,
                CheckKind::FileForeign => {
                    std::fs::write(corpus.join(file), build_minimal_xlsx(&[]))?
                }
            }
            checks.push(judge(kind, file, &names, load(profile, &ws)));
        }
    }
    Ok(checks)
}

fn judge(
    kind: CheckKind,
    file: &str,
    names: &[&str],
    result: Result<Vec<LoadedSource>, Refusal>,
) -> LiveCheck {
    let refusal = match result {
        Ok(_) => {
            return LiveCheck {
                kind,
                file: file.to_string(),
                observed: None,
                names_only_this_file: false,
                failure: Some(format!("{}: {file}", kind.not_refused())),
            };
        }
        Err(r) => r,
    };
    let what = refusal.what.as_str();
    let named = what.contains(&format!("{}: {file} (", kind.label()));
    let others = names
        .iter()
        .filter(|o| **o != file)
        .any(|o| what.contains(&format!("{o} (")));
    let names_only_this_file = named && !others;
    let failure = if refusal.exit_code != 8 {
        Some(format!("refusal exit {} is not 8", refusal.exit_code))
    } else if refusal.code != "FMINE-REF-502" || lookup_refusal_spec(refusal.code).is_none() {
        Some(format!(
            "refusal code {} is not FMINE-REF-502",
            refusal.code
        ))
    } else if !what.contains(&format!("1 of {} registered", names.len())) {
        Some(format!(
            "refusal does not count 1 of {} registered",
            names.len()
        ))
    } else if !named {
        Some(format!("{file} not named under \"{}\"", kind.label()))
    } else if others {
        Some(format!("refusal for {file} names another file"))
    } else {
        None
    };
    LiveCheck {
        kind,
        file: file.to_string(),
        observed: Some((refusal.code, refusal.exit_code)),
        names_only_this_file,
        failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::load_issuer_profile;
    use crate::{Lane, load_registered_sources, run_public_assurance, synthetic};
    use fmine_core::hash::sha256;
    use std::path::PathBuf;

    /// A fresh synthetic registered workspace (three files, issuer `synthetic`) and its profile.
    fn workspace(tag: &str) -> (PathBuf, IssuerProfile) {
        let ws = std::env::temp_dir().join(format!("fmine-live-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ws);
        synthetic::write_issuer_workspace(&ws).expect("write workspace");
        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        (ws, profile)
    }

    fn log(tag: &str, profile: &IssuerProfile, ws: &Path, checks: &[LiveCheck]) {
        let anchor = run_public_assurance(profile, ws)
            .expect("intact")
            .ledger_anchor
            .anchor_id()
            .to_string();
        let report = LiveReport {
            anchor: anchor.clone(),
            checks: checks.to_vec(),
        };
        let dir = std::env::temp_dir().join("live_dependency");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{anchor}-{tag}.jsonl")), report.to_jsonl());
        print!("{}", report.to_jsonl());
    }

    #[test]
    fn test_synthetic_registered_profile_digests_match_written_bytes() {
        let (ws, profile) = workspace("digests");
        assert_eq!(profile.sources.len(), 3);
        for s in &profile.sources {
            let bytes =
                std::fs::read(ws.join(&profile.corpus_dir).join(&s.file_name)).expect("written");
            assert_eq!(bytes.len() as u64, s.bytes, "{}", s.file_name);
            assert_eq!(sha256(&bytes).to_hex(), s.sha256_hex, "{}", s.file_name);
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_synthetic_registered_profile_runs_as_synthetic_lane() {
        let (ws, profile) = workspace("lane");
        let r = run_public_assurance(&profile, &ws).expect("intact");
        assert_eq!(r.lane, Lane::Synthetic);
        assert!(!r.gate_eligible());
        let json = r.to_json();
        assert!(!json.contains("\"lane\":\"public\""), "{json}");
        assert!(!json.contains("\"gate_eligible\":true"), "{json}");
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// Positive control: 3 files × {absent, 0 bytes, foreign xlsx} = 9 checks, every one refused as required.
    #[test]
    fn test_live_files_synthetic_all_refuse() {
        let (ws, profile) = workspace("all");
        let checks = prove_files(&profile, &ws, &ws.join("scratch"), &load_registered_sources)
            .expect("scratch workspaces");
        log("all", &profile, &ws, &checks);
        assert_eq!(checks.len(), 9);
        for c in &checks {
            assert!(c.pass(), "{}", c.to_json());
            assert_eq!(c.observed, Some(("FMINE-REF-502", 8)));
            assert!(c.names_only_this_file);
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_live_files_refusal_names_only_the_removed_file() {
        let (ws, profile) = workspace("names");
        let checks = prove_files(&profile, &ws, &ws.join("scratch"), &load_registered_sources)
            .expect("scratch workspaces");
        let c = checks
            .iter()
            .find(|c| c.kind == CheckKind::FileDeleted && c.file == synthetic::SYNTHETIC_AISC_FILE)
            .expect("check");
        assert!(c.pass() && c.names_only_this_file, "{}", c.to_json());
        // The same refusal, read directly: the counted prefix and the label, and no other file name.
        let scratch = ws.join("scratch").join("1-file_deleted");
        let r = load_registered_sources(&profile, &scratch).expect_err("refuses");
        assert!(r.what.contains("1 of 3 registered"), "{}", r.what);
        assert!(
            r.what.contains("missing: Synthetic_AISC.xlsx ("),
            "{}",
            r.what
        );
        for other in [
            synthetic::SYNTHETIC_STATS_FILE,
            synthetic::SYNTHETIC_LATER_STATS_FILE,
        ] {
            assert!(!r.what.contains(other), "{}", r.what);
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// M3, silent fallback: a loader that answers a refusal by putting the intact bytes back is caught for every
    /// file, by name.
    #[test]
    fn test_live_files_mutant_m3_silent_fallback_is_caught() {
        let (ws, profile) = workspace("m3");
        let intact = ws.join(&profile.corpus_dir);
        let fallback = |p: &IssuerProfile, root: &Path| match load_registered_sources(p, root) {
            Ok(v) => Ok(v),
            Err(_) => {
                for s in &p.sources {
                    let _ = std::fs::copy(
                        intact.join(&s.file_name),
                        root.join(&p.corpus_dir).join(&s.file_name),
                    );
                }
                load_registered_sources(p, root)
            }
        };
        let checks =
            prove_files(&profile, &ws, &ws.join("scratch"), &fallback).expect("scratch workspaces");
        log("m3", &profile, &ws, &checks);
        assert!(checks.iter().all(|c| !c.pass()));
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        for file in [
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_STATS_FILE,
            synthetic::SYNTHETIC_LATER_STATS_FILE,
        ] {
            assert!(
                failures.contains(&format!("file deletion did not refuse: {file}").as_str()),
                "{failures:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// M4, unregistered code: re-raising under a code the catalogue lacks degrades to FMINE-REF-000, exit 2.
    #[test]
    fn test_live_files_mutant_m4_unregistered_code_is_caught() {
        let (ws, profile) = workspace("m4");
        let reraise = |p: &IssuerProfile, root: &Path| {
            load_registered_sources(p, root)
                .map_err(|r| Refusal::from_code("FMINE-REF-5O2", r.what))
        };
        let checks =
            prove_files(&profile, &ws, &ws.join("scratch"), &reraise).expect("scratch workspaces");
        log("m4", &profile, &ws, &checks);
        assert_eq!(checks.len(), 9);
        for c in &checks {
            assert_eq!(
                c.failure.as_deref(),
                Some("refusal exit 2 is not 8"),
                "{}",
                c.to_json()
            );
            assert_eq!(c.observed, Some(("FMINE-REF-000", 2)));
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// An unreadable file is named. Today the loader files a read error under "missing" (`lib.rs`), and this
    /// test records that label.
    #[cfg(unix)]
    #[test]
    fn test_live_files_unreadable_file_is_named() {
        use std::os::unix::fs::PermissionsExt as _;
        let (ws, profile) = workspace("unreadable");
        let path = ws
            .join(&profile.corpus_dir)
            .join(synthetic::SYNTHETIC_AISC_FILE);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::read(&path).is_ok() {
            // Running as root: permissions do not stop the read, so there is nothing to observe. Say so.
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
            let _ = std::fs::remove_dir_all(&ws);
            println!(
                "{{\"test\":\"unreadable_file_is_named\",\"skipped\":\"chmod 000 file still readable (root)\"}}"
            );
            return;
        }
        let r = load_registered_sources(&profile, &ws).expect_err("refuses");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!((r.code, r.exit_code), ("FMINE-REF-502", 8));
        assert!(r.what.contains("1 of 3 registered"), "{}", r.what);
        assert!(
            r.what.contains("missing: Synthetic_AISC.xlsx ("),
            "{}",
            r.what
        );
        let _ = std::fs::remove_dir_all(&ws);
    }
}
