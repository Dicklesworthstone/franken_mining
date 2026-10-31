//! §28.4's budget-overrun protocol beyond the ceiling check (BD-012): the 80% warning, and the `BUDGET-AMEND-nnn`
//! record that a change adding code to a crate over its planned LOC needs.
//!
//! - **80% of planned**: a warning. §28.4.2 puts it in `fmine doctor`; until that verb reads the budget, `check-all`
//!   prints it.
//! - **Over planned**: a change that adds LOC to the crate needs an approved record in [`DELIVERY_DIR`], a file
//!   named `BUDGET-AMEND-nnn….md` whose `New planned` is at least the crate's actual LOC (§28.4.3). A change with net
//!   LOC ≤ 0 needs none: §28.4.4 lets one through even at the ceiling.
//! - **Over the ceiling**: [`crate::check_loc_budget`] fails, whatever changed.
//!
//! A change is the diff from a base revision to the working tree, untracked files included, measured as
//! `check_loc_budget` measures a crate (non-blank lines of its `.rs` files). CI passes the pushed range's base;
//! `check-all` passes `HEAD`, so it sees what is about to be committed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::LocReport;

/// Where amendment records live (§0.7: the delivery log, never the plan).
pub const DELIVERY_DIR: &str = "docs/delivery";

const PREFIX: &str = "BUDGET-AMEND-";

/// One `BUDGET-AMEND-nnn` record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Amendment {
    /// `BUDGET-AMEND-001`.
    pub id: String,
    pub crate_name: String,
    pub new_planned: usize,
    /// The `Approval` field reads `approved by <who>, YYYY-MM-DD`. Anything else, `PENDING` included, is not an
    /// approval.
    pub approved: bool,
}

/// What `check-amend` found when no rule failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmendReport {
    /// Net LOC per crate the change touched; `None` when the root is no git checkout, so nothing was measured.
    pub changed: Option<BTreeMap<String, i64>>,
    pub over_planned: usize,
    pub amendments: usize,
    pub approved: usize,
    pub warnings: Vec<String>,
}

impl AmendReport {
    /// `{"check":"check-amend","measured","changed","over_planned","amendments","approved","warnings"}`.
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"check-amend\",\"measured\":{},\"changed\":{},\"over_planned\":{},\"amendments\":{},\
             \"approved\":{},\"warnings\":{}}}",
            self.changed.is_some(),
            self.changed.as_ref().map_or(0, BTreeMap::len),
            self.over_planned,
            self.amendments,
            self.approved,
            self.warnings.len()
        )
    }
}

/// The value after `**<field>:**` on a `- **<field>:** value` line, without backticks.
fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let tag = format!("**{name}:**");
    text.lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("- ")?
                .trim()
                .strip_prefix(tag.as_str())
        })
        .map(|v| v.trim().trim_matches('`').trim())
}

fn is_approval(value: &str) -> bool {
    let Some((who, date)) = value
        .strip_prefix("approved by ")
        .and_then(|rest| rest.rsplit_once(", "))
    else {
        return false;
    };
    let d = date.as_bytes();
    !who.trim().is_empty()
        && d.len() == 10
        && d.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// Every `BUDGET-AMEND-nnn….md` in [`DELIVERY_DIR`]. A malformed name, a missing field or a repeated id refuses.
pub fn read_amendments(root: &Path) -> Result<Vec<Amendment>, Vec<String>> {
    let dir = root.join(DELIVERY_DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(PREFIX))
        .collect();
    names.sort();
    let (mut out, mut errors, mut ids) = (Vec::new(), Vec::new(), BTreeSet::new());
    for name in names {
        let digits = &name[PREFIX.len()..];
        if !name.ends_with(".md")
            || digits.len() < 3
            || !digits.as_bytes()[..3].iter().all(u8::is_ascii_digit)
        {
            errors.push(format!(
                "{DELIVERY_DIR}/{name}: not named BUDGET-AMEND-nnn….md"
            ));
            continue;
        }
        let id = format!("{PREFIX}{}", &digits[..3]);
        if !ids.insert(id.clone()) {
            errors.push(format!("{DELIVERY_DIR}/{name}: {id} is used twice"));
            continue;
        }
        let text = match fs::read_to_string(dir.join(&name)) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{DELIVERY_DIR}/{name}: {e}"));
                continue;
            }
        };
        let crate_name = field(&text, "Crate");
        let new_planned = field(&text, "New planned").map(|v| v.parse::<usize>());
        let approval = field(&text, "Approval");
        match (crate_name, new_planned, approval) {
            (Some(c), Some(Ok(n)), Some(a)) if !c.is_empty() => out.push(Amendment {
                id,
                crate_name: c.to_string(),
                new_planned: n,
                approved: is_approval(a),
            }),
            _ => errors.push(format!(
                "{DELIVERY_DIR}/{name}: needs `- **Crate:**`, `- **New planned:**` (digits only) and \
                 `- **Approval:**` lines"
            )),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Net LOC since `base` per crate whose `.rs` files changed, or `None` when `root` is no git work tree.
pub fn net_loc_since(
    root: &Path,
    base: &str,
) -> Result<Option<BTreeMap<String, i64>>, Vec<String>> {
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return Ok(None);
    }
    let one = |r: Result<String, String>| r.map_err(|e| vec![e]);
    let mut paths = one(git(root, &["diff", "--name-only", base, "--", "crates"]))?;
    paths.push_str(&one(git(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", "crates"],
    ))?);
    let crates: BTreeSet<&str> = paths
        .lines()
        .filter(|p| p.ends_with(".rs"))
        .filter_map(|p| p.strip_prefix("crates/")?.split('/').next())
        .collect();
    let mut net = BTreeMap::new();
    for name in crates {
        let now = crate::count_rust_loc(&root.join("crates").join(name));
        let listed = one(git(
            root,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                base,
                "--",
                &format!("crates/{name}"),
            ],
        ))?;
        let mut before = 0;
        for path in listed.lines().filter(|p| p.ends_with(".rs")) {
            let text = one(git(root, &["show", &format!("{base}:{path}")]))?;
            before += text.lines().filter(|l| !l.trim().is_empty()).count();
        }
        let (Ok(now), Ok(before)) = (i64::try_from(now), i64::try_from(before)) else {
            return Err(vec![format!(
                "crates/{name}: a LOC count does not fit an i64"
            )]);
        };
        net.insert(name.to_string(), now - before);
    }
    Ok(Some(net))
}

/// §28.4.2 and §28.4.3 over a LOC report, the amendments and the change's net LOC per crate.
pub fn evaluate(
    report: &LocReport,
    amendments: &[Amendment],
    changed: Option<BTreeMap<String, i64>>,
) -> Result<AmendReport, Vec<String>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let known: BTreeSet<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
    for a in amendments {
        if !known.contains(a.crate_name.as_str()) {
            errors.push(format!(
                "{}: crate {} is not in registries/loc_budget.toml",
                a.id, a.crate_name
            ));
        }
    }
    let mut over_planned = 0;
    for row in &report.rows {
        if !row.over_planned {
            if row.warn_80_pct {
                warnings.push(format!(
                    "{}: {} LOC is {}% of its planned {} (§28.4.2)",
                    row.name,
                    row.actual_loc,
                    row.actual_loc * 100 / row.planned_loc.max(1),
                    row.planned_loc
                ));
            }
            continue;
        }
        over_planned += 1;
        let ours: Vec<&Amendment> = amendments
            .iter()
            .filter(|a| a.crate_name == row.name)
            .collect();
        if ours
            .iter()
            .any(|a| a.approved && a.new_planned >= row.actual_loc)
        {
            continue;
        }
        let pending: Vec<&str> = ours.iter().map(|a| a.id.as_str()).collect();
        let pending = if pending.is_empty() {
            String::new()
        } else {
            format!(" (not approved or too small: {})", pending.join(", "))
        };
        let added = changed
            .as_ref()
            .and_then(|c| c.get(&row.name))
            .copied()
            .unwrap_or(0);
        let rule = format!(
            "an approved {PREFIX}nnn record in {DELIVERY_DIR}/ with New planned ≥ {} (§28.4.3){pending}",
            row.actual_loc
        );
        if added > 0 {
            errors.push(format!(
                "{}: {} LOC is over its planned {} and this change adds {added}; it needs {rule}",
                row.name, row.actual_loc, row.planned_loc
            ));
        } else {
            warnings.push(format!(
                "{}: {} LOC is over its planned {}; a change adding to it needs {rule}",
                row.name, row.actual_loc, row.planned_loc
            ));
        }
    }
    if changed.is_none() {
        warnings.push(
            "not a git checkout, so no change was measured; CI checks the pushed range".to_string(),
        );
    }
    if errors.is_empty() {
        Ok(AmendReport {
            changed,
            over_planned,
            amendments: amendments.len(),
            approved: amendments.iter().filter(|a| a.approved).count(),
            warnings,
        })
    } else {
        Err(errors)
    }
}

/// [`evaluate`] over `root`'s budget, records and change since `base`. With `require_git`, a root that is no git
/// work tree refuses instead of warning: CI must measure the range.
pub fn check_amend(root: &Path, base: &str, require_git: bool) -> Result<AmendReport, Vec<String>> {
    let report = crate::check_loc_budget(root)?;
    let amendments = read_amendments(root)?;
    let changed = net_loc_since(root, base)?;
    if require_git && changed.is_none() {
        return Err(vec![format!(
            "{} is not a git checkout; cannot measure the change since {base}",
            root.display()
        )]);
    }
    evaluate(&report, &amendments, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrateBudgetRow;

    fn row(name: &str, planned: usize, actual: usize) -> CrateBudgetRow {
        CrateBudgetRow {
            name: name.to_string(),
            layer: "V".to_string(),
            planned_loc: planned,
            ceiling_loc: planned * 115 / 100,
            actual_loc: actual,
            warn_80_pct: actual * 100 >= planned * 80,
            over_planned: actual > planned,
            over_ceiling: actual > planned * 115 / 100,
        }
    }

    fn report(rows: Vec<CrateBudgetRow>) -> LocReport {
        LocReport {
            total_planned: rows.iter().map(|r| r.planned_loc).sum(),
            total_ceiling: rows.iter().map(|r| r.ceiling_loc).sum(),
            total_actual: rows.iter().map(|r| r.actual_loc).sum(),
            rows,
        }
    }

    fn amend(id: &str, krate: &str, new_planned: usize, approved: bool) -> Amendment {
        Amendment {
            id: id.to_string(),
            crate_name: krate.to_string(),
            new_planned,
            approved,
        }
    }

    fn changed(pairs: &[(&str, i64)]) -> Option<BTreeMap<String, i64>> {
        Some(pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())
    }

    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A scratch root holding the real budget registry, so `check_loc_budget`'s totals hold.
    fn scratch(tag: &str) -> (std::path::PathBuf, Cleanup) {
        let base = std::env::temp_dir().join(format!("fmine-loc-{tag}-{}", std::process::id()));
        let cleanup = Cleanup(base.clone());
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("registries")).expect("registries");
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registries/loc_budget.toml");
        fs::copy(real, base.join("registries/loc_budget.toml")).expect("budget");
        (base, cleanup)
    }

    fn lines(n: usize) -> String {
        (0..n)
            .map(|i| format!("pub const L{i}: u32 = {i};\n"))
            .collect()
    }

    #[test]
    fn test_over_planned_change_needs_an_approved_amendment() {
        let rep = report(vec![
            row("fmine-public", 1000, 1100),
            row("fmine-io", 1000, 100),
        ]);
        // Adding to a crate over planned with no record refuses, naming the rule.
        let err = evaluate(&rep, &[], changed(&[("fmine-public", 5)])).expect_err("no record");
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(
            err[0]
                .contains("fmine-public: 1100 LOC is over its planned 1000 and this change adds 5"),
            "{err:?}"
        );
        assert!(err[0].contains("New planned ≥ 1100 (§28.4.3)"), "{err:?}");
        // A pending record, or an approved one below the actual, does not cover it and is named.
        for a in [
            amend("BUDGET-AMEND-001", "fmine-public", 1200, false),
            amend("BUDGET-AMEND-001", "fmine-public", 1099, true),
        ] {
            let err =
                evaluate(&rep, &[a], changed(&[("fmine-public", 5)])).expect_err("not covered");
            assert!(
                err[0].ends_with("(not approved or too small: BUDGET-AMEND-001)"),
                "{err:?}"
            );
        }
        // An approved record at or above the actual covers it.
        let ok = evaluate(
            &rep,
            &[amend("BUDGET-AMEND-001", "fmine-public", 1100, true)],
            changed(&[("fmine-public", 5)]),
        )
        .expect("covered");
        assert_eq!((ok.over_planned, ok.amendments, ok.approved), (1, 1, 1));
        // Net LOC ≤ 0, or no change at all, passes with a warning; so does a change to another crate.
        for c in [
            changed(&[("fmine-public", 0)]),
            changed(&[("fmine-public", -40)]),
            changed(&[("fmine-io", 9)]),
        ] {
            let ok = evaluate(&rep, &[], c).expect("net ≤ 0 needs no record");
            assert_eq!(ok.warnings.len(), 1, "{:?}", ok.warnings);
            assert!(
                ok.warnings[0].contains("a change adding to it needs"),
                "{:?}",
                ok.warnings
            );
        }
        // A record for a crate the budget does not list refuses.
        let err = evaluate(
            &rep,
            &[amend("BUDGET-AMEND-002", "fmine-nope", 1, true)],
            changed(&[]),
        )
        .expect_err("unknown crate");
        assert_eq!(
            err,
            ["BUDGET-AMEND-002: crate fmine-nope is not in registries/loc_budget.toml"]
        );
    }

    #[test]
    fn test_eighty_percent_warns_and_no_git_says_so() {
        let rep = report(vec![
            row("fmine-calib", 1000, 800),
            row("fmine-io", 1000, 799),
        ]);
        let ok = evaluate(&rep, &[], changed(&[("fmine-calib", 50)])).expect("80% only warns");
        assert_eq!(
            ok.warnings,
            ["fmine-calib: 800 LOC is 80% of its planned 1000 (§28.4.2)"]
        );
        let ok = evaluate(&rep, &[], None).expect("unmeasured only warns");
        assert_eq!(ok.warnings.len(), 2, "{:?}", ok.warnings);
        assert!(
            ok.warnings[1].starts_with("not a git checkout"),
            "{:?}",
            ok.warnings
        );
        assert!(
            ok.log_line().contains("\"measured\":false"),
            "{}",
            ok.log_line()
        );
    }

    #[test]
    fn test_amendment_records_parse_and_malformed_ones_refuse() {
        let (root, _c) = scratch("records");
        let dir = root.join(DELIVERY_DIR);
        fs::create_dir_all(&dir).expect("delivery");
        let record = |krate: &str, n: &str, approval: &str| {
            format!(
                "# {PREFIX}nnn\n\n- **Crate:** `{krate}`\n- **New planned:** {n}\n- **Scope:** x\n\
                 - **Approval:** {approval}\n"
            )
        };
        fs::write(
            dir.join("BUDGET-AMEND-001-fmine-public.md"),
            record("fmine-public", "22000", "PENDING"),
        )
        .expect("w");
        fs::write(
            dir.join("BUDGET-AMEND-002.md"),
            record("fmine-io", "47000", "approved by Owner, 2026-11-02"),
        )
        .expect("w");
        fs::write(dir.join("README.md"), "not a record").expect("w");
        let got = read_amendments(&root).expect("two records");
        assert_eq!(
            got,
            [
                amend("BUDGET-AMEND-001", "fmine-public", 22000, false),
                amend("BUDGET-AMEND-002", "fmine-io", 47000, true),
            ]
        );
        for bad in [
            "approved by , 2026-11-02",
            "approved by Owner, 2026-1-02",
            "approved by Owner, 2026-11-022",
            "approved by Owner 2026-11-02",
            "yes",
        ] {
            assert!(!is_approval(bad), "{bad}");
        }
        // A repeated id, a malformed name and a missing field each refuse, naming the file.
        fs::write(
            dir.join("BUDGET-AMEND-001-again.md"),
            record("fmine-io", "1", "PENDING"),
        )
        .expect("w");
        fs::write(
            dir.join("BUDGET-AMEND-x.md"),
            record("fmine-io", "1", "PENDING"),
        )
        .expect("w");
        fs::write(
            dir.join("BUDGET-AMEND-003.md"),
            record("fmine-io", "22,000", "PENDING"),
        )
        .expect("w");
        let err = read_amendments(&root).expect_err("three bad records");
        assert_eq!(err.len(), 3, "{err:?}");
        assert!(
            err[0].contains("BUDGET-AMEND-001-fmine-public.md: BUDGET-AMEND-001 is used twice"),
            "{err:?}"
        );
        assert!(err[1].contains("BUDGET-AMEND-003.md: needs"), "{err:?}");
        assert!(err[2].contains("BUDGET-AMEND-x.md: not named"), "{err:?}");
    }

    /// BD-012's done-when: a synthetic overrun blocks the merge. Over the ceiling, `check_loc_budget` (and so
    /// `check-all`) fails; over planned, a commit adding to the crate fails `check-amend` until a record is approved.
    #[test]
    fn test_synthetic_overrun_blocks_a_merge() {
        let (root, _c) = scratch("overrun");
        let src = root.join("crates/fmine-array/src");
        fs::create_dir_all(&src).expect("src");
        // fmine-array: planned 6000, ceiling 7000 (registries/loc_budget.toml).
        fs::write(src.join("lib.rs"), lines(7001)).expect("w");
        let err = crate::check_loc_budget(&root).expect_err("over the ceiling");
        assert!(
            err.iter()
                .any(|e| e == "LOC ceiling overrun in fmine-array: actual 7001 > ceiling 7000"),
            "{err:?}"
        );

        fs::write(src.join("lib.rs"), lines(6100)).expect("w");
        let g = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .output()
                .expect("git");
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        g(&["init", "-q"]);
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "base"]);
        // Nothing changed since HEAD: over planned only warns.
        let ok = check_amend(&root, "HEAD", true).expect("no change");
        assert_eq!(ok.changed, Some(BTreeMap::new()));
        assert!(
            ok.warnings
                .iter()
                .any(|w| w.starts_with("fmine-array: 6100 LOC is over its planned 6000"))
        );
        // A new untracked file adds 20 lines: refused.
        fs::write(src.join("more.rs"), lines(20)).expect("w");
        let err = check_amend(&root, "HEAD", true).expect_err("adds to an over-planned crate");
        assert!(
            err[0]
                .contains("fmine-array: 6120 LOC is over its planned 6000 and this change adds 20"),
            "{err:?}"
        );
        // Committed, it is refused over the range from the base commit, as CI measures it.
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "grow"]);
        assert!(check_amend(&root, "HEAD~1", true).is_err());
        // An approved record covering 6120 lets it through.
        fs::create_dir_all(root.join(DELIVERY_DIR)).expect("delivery");
        fs::write(
            root.join(DELIVERY_DIR).join("BUDGET-AMEND-001.md"),
            "- **Crate:** `fmine-array`\n- **New planned:** 6200\n- **Approval:** approved by Owner, 2026-11-02\n",
        )
        .expect("w");
        assert!(check_amend(&root, "HEAD~1", true).is_ok());
    }

    #[test]
    fn test_net_loc_counts_non_blank_lines_and_a_non_repo_refuses_only_when_required() {
        let (root, _c) = scratch("net");
        let src = root.join("crates/fmine-io/src");
        fs::create_dir_all(&src).expect("src");
        // Ten code lines with two blank lines between, at the base too: blank lines never count.
        fs::write(src.join("lib.rs"), format!("{}\n\n{}", lines(5), lines(5))).expect("w");
        // Not yet a repository: nothing measured; CI mode refuses.
        assert_eq!(net_loc_since(&root, "HEAD").expect("no repo"), None);
        assert!(check_amend(&root, "HEAD", false).is_ok());
        let err = check_amend(&root, "HEAD", true).expect_err("CI must measure");
        assert!(
            err[0].ends_with("is not a git checkout; cannot measure the change since HEAD"),
            "{err:?}"
        );
        let g = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .output()
                .expect("git");
            assert!(out.status.success(), "{args:?}");
        };
        g(&["init", "-q"]);
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "ten"]);
        // lib.rs goes from ten code lines to four, with three blank lines between that do not count; an empty new
        // file and a README add nothing: net −6.
        fs::write(
            src.join("lib.rs"),
            format!("{}\n\n\n{}", lines(2), lines(2)),
        )
        .expect("w");
        fs::write(src.join("extra.rs"), lines(0)).expect("w");
        fs::write(
            root.join("crates/fmine-io/README.md"),
            "docs do not count\n",
        )
        .expect("w");
        let net = net_loc_since(&root, "HEAD")
            .expect("measured")
            .expect("repo");
        assert_eq!(net, BTreeMap::from([("fmine-io".to_string(), -6)]));
    }
}
