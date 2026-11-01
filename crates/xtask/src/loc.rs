//! §28.4's budget-overrun protocol (BD-012): the 80% warning, the `BUDGET-AMEND-nnn` record a crate over its
//! planned LOC needs before a change adds to it, and the ceiling.
//!
//! - **80% of planned**: a warning. §28.4.2 puts it in `fmine doctor`; until that verb reads the budget, `check-all`
//!   prints it.
//! - **Over planned**: a change that adds LOC to the crate needs an approved record in [`DELIVERY_DIR`], a file
//!   named `BUDGET-AMEND-nnn….md` whose `New planned` is at least the crate's actual LOC (§28.4.3).
//! - **Over the ceiling**: a change that adds LOC to the crate fails, record or not (§28.4.4). A change with net
//!   LOC ≤ 0 passes at both levels, and a change that leaves the crate alone is not blocked by it.
//! - **The budget itself**: a change to a crate's row in `registries/loc_budget.toml`, against the base's, needs
//!   an approved record whose `New planned` is the new number, so a change cannot move budget between crates.
//!
//! A change is the diff from the merge-base of a base revision and `HEAD` to the working tree, untracked files
//! included, over the `.rs` files git lists (tracked, or untracked and not ignored), counting non-blank lines.
//! CI passes the pushed range's base; `check-all` passes `HEAD`, so it sees what is about to be committed. A root
//! that is not a git work tree's top level is not measured: then the ceiling fails as it did before this check,
//! and `--base` refuses.
//!
//! Known limits. The approval is a line in a file: the check cannot tell who wrote it, so an approval added by the
//! change under test is named in a warning for a human to confirm. A crate directory renamed in the change counts
//! as wholly added, because git lists only the new paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::LocReport;

/// Where amendment records live (§0.7: the delivery log, never the plan).
pub const DELIVERY_DIR: &str = "docs/delivery";

/// The budget registry, as `check_loc_budget` reads it.
pub const BUDGET_PATH: &str = "registries/loc_budget.toml";

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

/// What changed since the base.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// Net LOC per crate whose `.rs` files changed.
    pub net: BTreeMap<String, i64>,
    /// Each crate's `(planned, ceiling)` in the base's budget registry; `None` when the base has none, or no row in
    /// it parses.
    pub base_budget: Option<BTreeMap<String, (usize, usize)>>,
    /// Ids of the records that read approved now and did not at the base.
    pub approvals_added: Vec<String>,
}

/// What `check-amend` found when no rule failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmendReport {
    /// Whether a change was measured at all.
    pub measured: bool,
    pub changed: usize,
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
            self.measured,
            self.changed,
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
    !who.trim().is_empty() && crate::is_iso_date(date)
}

/// The record id a file name gives, or `None` when it is not `BUDGET-AMEND-nnn….md`.
fn record_id(name: &str) -> Option<String> {
    let digits = name.strip_prefix(PREFIX)?;
    let ok = name.ends_with(".md")
        && digits.len() >= 3
        && digits.as_bytes()[..3].iter().all(u8::is_ascii_digit);
    ok.then(|| format!("{PREFIX}{}", &digits[..3]))
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
        let Some(id) = record_id(&name) else {
            errors.push(format!(
                "{DELIVERY_DIR}/{name}: not named BUDGET-AMEND-nnn….md"
            ));
            continue;
        };
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

/// Each crate's `(planned, ceiling)` in a budget registry's text.
pub fn budget_rows(text: &str) -> BTreeMap<String, (usize, usize)> {
    let mut rows = BTreeMap::new();
    let mut cur: Option<(String, usize, usize)> = None;
    for line in text.lines().map(str::trim) {
        if line == "[[crate]]" {
            if let Some((n, p, c)) = cur.take() {
                rows.insert(n, (p, c));
            }
            cur = Some((String::new(), 0, 0));
        } else if let Some((n, p, c)) = cur.as_mut() {
            if let Some(v) = crate::strip_kv_str(line, "name") {
                *n = v;
            } else if let Some(v) = crate::strip_kv_usize(line, "planned_loc") {
                *p = v;
            } else if let Some(v) = crate::strip_kv_usize(line, "ceiling_loc") {
                *c = v;
            }
        }
    }
    if let Some((n, p, c)) = cur {
        rows.insert(n, (p, c));
    }
    rows.remove("");
    rows
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

/// The NUL-separated paths a `-z` git listing prints.
fn z_paths(listing: &str) -> impl Iterator<Item = &str> {
    listing.split('\0').filter(|p| !p.is_empty())
}

fn non_blank(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// The change since the merge-base of `base` and `HEAD`, or `None` when `root` is not a git work tree's top level.
pub fn measure(root: &Path, base: &str) -> Result<Option<Change>, Vec<String>> {
    let Ok(top) = git(root, &["rev-parse", "--show-toplevel"]) else {
        return Ok(None);
    };
    let same = |a: &Path, b: &Path| matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y);
    if !same(Path::new(top.trim()), root) {
        return Ok(None);
    }
    let one = |r: Result<String, String>| r.map_err(|e| vec![e]);
    let mb = one(git(root, &["merge-base", base, "HEAD"]))?
        .trim()
        .to_string();
    let changed_under = |dir: &str| -> Result<String, Vec<String>> {
        let mut paths = one(git(root, &["diff", "-z", "--name-only", &mb, "--", dir]))?;
        paths.push_str(&one(git(
            root,
            &[
                "ls-files",
                "-z",
                "--others",
                "--exclude-standard",
                "--",
                dir,
            ],
        ))?);
        Ok(paths)
    };

    let paths = changed_under("crates")?;
    let crates: BTreeSet<&str> = z_paths(&paths)
        .filter(|p| p.ends_with(".rs"))
        .filter_map(|p| p.strip_prefix("crates/")?.split('/').next())
        .collect();
    let mut net = BTreeMap::new();
    for name in crates {
        let dir = format!("crates/{name}");
        let listed = one(git(
            root,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
                "--",
                &dir,
            ],
        ))?;
        let mut now = 0;
        for path in z_paths(&listed).filter(|p| p.ends_with(".rs")) {
            match fs::read_to_string(root.join(path)) {
                Ok(text) => now += non_blank(&text),
                // Deleted in the working tree but still in the index: it adds nothing.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(vec![format!("{path}: {e}")]),
            }
        }
        let listed = one(git(
            root,
            &["ls-tree", "-r", "-z", "--name-only", &mb, "--", &dir],
        ))?;
        let mut before = 0;
        for path in z_paths(&listed).filter(|p| p.ends_with(".rs")) {
            before += non_blank(&one(git(root, &["show", &format!("{mb}:{path}")]))?);
        }
        let (Ok(now), Ok(before)) = (i64::try_from(now), i64::try_from(before)) else {
            return Err(vec![format!("{dir}: a LOC count does not fit an i64")]);
        };
        net.insert(name.to_string(), now - before);
    }

    let base_budget = git(root, &["show", &format!("{mb}:{BUDGET_PATH}")])
        .ok()
        .map(|t| budget_rows(&t))
        .filter(|rows| !rows.is_empty());

    let records = changed_under(DELIVERY_DIR)?;
    let mut approvals_added = Vec::new();
    for path in z_paths(&records) {
        let Some(name) = path.rsplit('/').next() else {
            continue;
        };
        let Some(id) = record_id(name) else {
            continue;
        };
        let approved = |text: &str| field(text, "Approval").is_some_and(is_approval);
        let now = fs::read_to_string(root.join(path)).is_ok_and(|t| approved(&t));
        let then = git(root, &["show", &format!("{mb}:{path}")]).is_ok_and(|t| approved(&t));
        if now && !then {
            approvals_added.push(id);
        }
    }
    Ok(Some(Change {
        net,
        base_budget,
        approvals_added,
    }))
}

/// §28.4.2 to §28.4.4 over a LOC report, the amendments and the change since the base (`None`: not measured).
pub fn evaluate(
    report: &LocReport,
    amendments: &[Amendment],
    change: Option<&Change>,
) -> Result<AmendReport, Vec<String>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let known: BTreeSet<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
    for a in amendments {
        if !known.contains(a.crate_name.as_str()) {
            errors.push(format!(
                "{}: crate {} is not in {BUDGET_PATH}",
                a.id, a.crate_name
            ));
        }
    }
    let approved_for = |name: &str| -> Vec<&Amendment> {
        amendments
            .iter()
            .filter(|a| a.approved && a.crate_name == name)
            .collect()
    };
    let named = |name: &str| -> String {
        let ids: Vec<&str> = amendments
            .iter()
            .filter(|a| a.crate_name == name)
            .map(|a| a.id.as_str())
            .collect();
        if ids.is_empty() {
            String::new()
        } else {
            format!(" (not approved or too small: {})", ids.join(", "))
        }
    };
    let mut over_planned = 0;
    for row in &report.rows {
        let name = row.name.as_str();
        let added = change.and_then(|c| c.net.get(name)).copied().unwrap_or(0);

        if let Some(base) = change.and_then(|c| c.base_budget.as_ref()) {
            let was = base.get(name).copied();
            if was != Some((row.planned_loc, row.ceiling_loc))
                && !approved_for(name)
                    .iter()
                    .any(|a| a.new_planned == row.planned_loc)
            {
                let from = was.map_or("no row".to_string(), |(p, c)| format!("{p}/{c}"));
                errors.push(format!(
                    "{name}: its budget row changed from {from} to {}/{} without an approved {PREFIX}nnn record \
                     whose New planned is {} (§28.4.3){}",
                    row.planned_loc,
                    row.ceiling_loc,
                    row.planned_loc,
                    named(name)
                ));
            }
        }

        if row.over_ceiling {
            let what = format!(
                "{name}: {} LOC is over its ceiling {}",
                row.actual_loc, row.ceiling_loc
            );
            match change {
                None => errors.push(format!(
                    "{what}, and no change was measured; a change may only remove code from it (§28.4.4)"
                )),
                Some(_) if added > 0 => errors.push(format!(
                    "{what} and this change adds {added}; only deletions and refactors with net LOC ≤ 0 may \
                     touch it (§28.4.4)"
                )),
                Some(_) => warnings.push(format!(
                    "{what}; only changes with net LOC ≤ 0 may touch it (§28.4.4)"
                )),
            }
        }

        if !row.over_planned {
            if row.warn_80_pct {
                warnings.push(format!(
                    "{name}: {} LOC is {}% of its planned {} (§28.4.2)",
                    row.actual_loc,
                    row.actual_loc * 100 / row.planned_loc.max(1),
                    row.planned_loc
                ));
            }
            continue;
        }
        over_planned += 1;
        if approved_for(name)
            .iter()
            .any(|a| a.new_planned >= row.actual_loc)
        {
            continue;
        }
        let rule = format!(
            "an approved {PREFIX}nnn record in {DELIVERY_DIR}/ with New planned ≥ {} (§28.4.3){}",
            row.actual_loc,
            named(name)
        );
        if added > 0 {
            errors.push(format!(
                "{name}: {} LOC is over its planned {} and this change adds {added}; it needs {rule}",
                row.actual_loc, row.planned_loc
            ));
        } else {
            warnings.push(format!(
                "{name}: {} LOC is over its planned {}; a change adding to it needs {rule}",
                row.actual_loc, row.planned_loc
            ));
        }
    }
    match change {
        None => warnings.push(
            "not a git work tree's top level, so no change was measured; CI checks the pushed range"
                .to_string(),
        ),
        Some(c) => {
            for id in &c.approvals_added {
                warnings.push(format!(
                    "{id}: its approval is part of this change; confirm the owner wrote it"
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(AmendReport {
            measured: change.is_some(),
            changed: change.map_or(0, |c| c.net.len()),
            over_planned,
            amendments: amendments.len(),
            approved: amendments.iter().filter(|a| a.approved).count(),
            warnings,
        })
    } else {
        Err(errors)
    }
}

/// [`evaluate`] over `root`'s budget, records and change since `base`. With `require_git`, a root that is not a
/// git work tree's top level refuses instead of warning: CI must measure the range.
pub fn check_amend(root: &Path, base: &str, require_git: bool) -> Result<AmendReport, Vec<String>> {
    let report = crate::check_loc_budget(root)?;
    let amendments = read_amendments(root)?;
    let change = measure(root, base)?;
    if require_git && change.is_none() {
        return Err(vec![format!(
            "{} is not a git work tree's top level; cannot measure the change since {base}",
            root.display()
        )]);
    }
    evaluate(&report, &amendments, change.as_ref())
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

    fn changed(pairs: &[(&str, i64)]) -> Change {
        Change {
            net: pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            ..Change::default()
        }
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
        fs::copy(real, base.join(BUDGET_PATH)).expect("budget");
        (base, cleanup)
    }

    fn git_in(root: &Path) -> impl Fn(&[&str]) + '_ {
        move |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .output()
                .expect("git");
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
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
        let err =
            evaluate(&rep, &[], Some(&changed(&[("fmine-public", 5)]))).expect_err("no record");
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
            let err = evaluate(&rep, &[a], Some(&changed(&[("fmine-public", 5)])))
                .expect_err("not covered");
            assert!(
                err[0].ends_with("(not approved or too small: BUDGET-AMEND-001)"),
                "{err:?}"
            );
        }
        // An approved record at or above the actual covers it.
        let ok = evaluate(
            &rep,
            &[amend("BUDGET-AMEND-001", "fmine-public", 1100, true)],
            Some(&changed(&[("fmine-public", 5)])),
        )
        .expect("covered");
        assert_eq!((ok.over_planned, ok.amendments, ok.approved), (1, 1, 1));
        // Net LOC ≤ 0, or no change at all, passes with a warning; so does a change to another crate.
        for c in [
            changed(&[("fmine-public", 0)]),
            changed(&[("fmine-public", -40)]),
            changed(&[("fmine-io", 9)]),
        ] {
            let ok = evaluate(&rep, &[], Some(&c)).expect("net ≤ 0 needs no record");
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
            Some(&changed(&[])),
        )
        .expect_err("unknown crate");
        assert_eq!(
            err,
            ["BUDGET-AMEND-002: crate fmine-nope is not in registries/loc_budget.toml"]
        );
    }

    #[test]
    fn test_ceiling_blocks_only_changes_that_add_to_the_crate() {
        // planned 1000, ceiling 1150.
        let rep = report(vec![
            row("fmine-public", 1000, 1151),
            row("fmine-io", 1000, 100),
        ]);
        let covering = [amend("BUDGET-AMEND-001", "fmine-public", 2000, true)];
        // Adding to it fails even with an approved record: the ceiling moves only with the budget row.
        let err = evaluate(&rep, &covering, Some(&changed(&[("fmine-public", 1)])))
            .expect_err("over the ceiling");
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(
            err[0].starts_with(
                "fmine-public: 1151 LOC is over its ceiling 1150 and this change adds 1"
            ),
            "{err:?}"
        );
        // A deletion, or a change to another crate, passes with a warning.
        for c in [
            changed(&[("fmine-public", -3)]),
            changed(&[("fmine-io", 50)]),
        ] {
            let ok = evaluate(&rep, &covering, Some(&c)).expect("net ≤ 0, or untouched");
            assert!(
                ok.warnings[0].contains("only changes with net LOC ≤ 0 may touch it (§28.4.4)"),
                "{:?}",
                ok.warnings
            );
        }
        // Unmeasured, the ceiling fails as before this check.
        let err = evaluate(&rep, &covering, None).expect_err("unmeasured");
        assert!(err[0].contains("no change was measured"), "{err:?}");
    }

    #[test]
    fn test_budget_row_change_needs_an_amendment_with_the_new_number() {
        // The change moves 4000 lines of budget from fmine-calib to fmine-public, keeping the totals; both crates
        // stay under 80% of their new planned, so only the budget rule speaks.
        let mut public = row("fmine-public", 22000, 17000);
        public.ceiling_loc = 25000;
        let mut calib = row("fmine-calib", 14000, 6469);
        calib.ceiling_loc = 17000;
        let rep = report(vec![public, calib]);
        let mut c = changed(&[("fmine-public", 1000)]);
        c.base_budget = Some(BTreeMap::from([
            ("fmine-public".to_string(), (18000, 21000)),
            ("fmine-calib".to_string(), (18000, 21000)),
        ]));
        let err = evaluate(&rep, &[], Some(&c)).expect_err("budget moved");
        assert_eq!(err.len(), 2, "{err:?}");
        assert!(
            err[0].starts_with(
                "fmine-public: its budget row changed from 18000/21000 to 22000/25000 without an approved"
            ),
            "{err:?}"
        );
        assert!(
            err[1].starts_with("fmine-calib: its budget row changed"),
            "{err:?}"
        );
        // An approved record must name the new number exactly, one per changed row.
        let wrong = [
            amend("BUDGET-AMEND-001", "fmine-public", 23000, true),
            amend("BUDGET-AMEND-002", "fmine-calib", 14000, true),
        ];
        let err = evaluate(&rep, &wrong, Some(&c)).expect_err("wrong number");
        assert_eq!(err.len(), 1, "{err:?}");
        let right = [
            amend("BUDGET-AMEND-001", "fmine-public", 22000, true),
            amend("BUDGET-AMEND-002", "fmine-calib", 14000, true),
        ];
        let ok = evaluate(&rep, &right, Some(&c)).expect("both rows approved");
        assert!(ok.warnings.is_empty(), "{:?}", ok.warnings);
        // An approval that is part of the change is named for a human.
        c.approvals_added = vec!["BUDGET-AMEND-001".to_string()];
        let ok = evaluate(&rep, &right, Some(&c)).expect("approved");
        assert_eq!(
            ok.warnings,
            ["BUDGET-AMEND-001: its approval is part of this change; confirm the owner wrote it"]
        );
        // A row the base lacks is a change too.
        c.base_budget = Some(BTreeMap::from([(
            "fmine-public".to_string(),
            (22000, 25000),
        )]));
        let err = evaluate(&rep, &right[..1], Some(&c)).expect_err("new row");
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(
            err[0].starts_with("fmine-calib: its budget row changed from no row to 14000/17000"),
            "{err:?}"
        );
    }

    /// The budget is compared with the merge-base's registry, so a change moving lines between crates refuses.
    #[test]
    fn test_budget_moved_between_crates_in_a_change_refuses() {
        let (root, _c) = scratch("moved");
        let src = root.join("crates/fmine-io/src");
        fs::create_dir_all(&src).expect("src");
        fs::write(src.join("lib.rs"), lines(10)).expect("w");
        let g = git_in(&root);
        g(&["init", "-q"]);
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "base"]);
        let path = root.join(BUDGET_PATH);
        let text = fs::read_to_string(&path).expect("budget");
        let (array, io) = (
            "name = \"fmine-array\"\nlayer = \"S0\"\nsubsystem = \"Bedrock\"\nphase = \"A\"\nplanned_loc = 6000\n",
            "name = \"fmine-io\"\nlayer = \"V\"\nsubsystem = \"Portal\"\nphase = \"A\"\nplanned_loc = 40000\n",
        );
        assert_eq!(
            (text.matches(array).count(), text.matches(io).count()),
            (1, 1)
        );
        let moved = text
            .replace(array, &array.replace("6000", "6100"))
            .replace(io, &io.replace("40000", "39900"));
        fs::write(&path, moved).expect("w");
        // The totals still hold, so `check_loc_budget` passes; the change to both rows refuses.
        assert!(crate::check_loc_budget(&root).is_ok());
        let err = check_amend(&root, "HEAD", true).expect_err("budget moved");
        assert_eq!(err.len(), 2, "{err:?}");
        assert!(
            err[0].starts_with("fmine-array: its budget row changed from 6000/7000 to 6100/7000"),
            "{err:?}"
        );
        assert!(
            err[1].starts_with("fmine-io: its budget row changed from 40000/46000 to 39900/46000"),
            "{err:?}"
        );
    }

    #[test]
    fn test_eighty_percent_warns_and_no_git_says_so() {
        let rep = report(vec![
            row("fmine-calib", 1000, 800),
            row("fmine-io", 1000, 799),
        ]);
        let ok =
            evaluate(&rep, &[], Some(&changed(&[("fmine-calib", 50)]))).expect("80% only warns");
        assert_eq!(
            ok.warnings,
            ["fmine-calib: 800 LOC is 80% of its planned 1000 (§28.4.2)"]
        );
        let ok = evaluate(&rep, &[], None).expect("unmeasured only warns");
        assert_eq!(ok.warnings.len(), 2, "{:?}", ok.warnings);
        assert!(
            ok.warnings[1].starts_with("not a git work tree's top level"),
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

    #[test]
    fn test_budget_rows_read_the_registry() {
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registries/loc_budget.toml");
        let rows = budget_rows(&fs::read_to_string(real).expect("budget"));
        assert!(rows.len() >= 59, "{}", rows.len());
        assert_eq!(rows.get("fmine-array"), Some(&(6000, 7000)));
        assert_eq!(rows.values().map(|r| r.0).sum::<usize>(), 1_128_000);
    }

    /// BD-012's done-when: a synthetic overrun blocks the merge. Over planned, a commit adding to the crate fails
    /// `check-amend` until a record is approved; over the ceiling, it fails whatever the records say, while a
    /// deletion passes.
    #[test]
    fn test_synthetic_overrun_blocks_a_merge() {
        let (root, _c) = scratch("overrun");
        let src = root.join("crates/fmine-array/src");
        fs::create_dir_all(&src).expect("src");
        // fmine-array: planned 6000, ceiling 7000 (registries/loc_budget.toml).
        fs::write(src.join("lib.rs"), lines(7001)).expect("w");
        // Not yet a repository: the ceiling fails, as it did before this check.
        let err = check_amend(&root, "HEAD", false).expect_err("over the ceiling, unmeasured");
        assert!(
            err.iter()
                .any(|e| e.starts_with("fmine-array: 7001 LOC is over its ceiling 7000")),
            "{err:?}"
        );

        fs::write(src.join("lib.rs"), lines(6100)).expect("w");
        let g = git_in(&root);
        g(&["init", "-q"]);
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "base"]);
        // Nothing changed since HEAD: over planned only warns.
        let ok = check_amend(&root, "HEAD", true).expect("no change");
        assert_eq!((ok.measured, ok.changed), (true, 0));
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
        // An approved record covering 6120 lets it through, and is named as part of the change.
        fs::create_dir_all(root.join(DELIVERY_DIR)).expect("delivery");
        fs::write(
            root.join(DELIVERY_DIR).join("BUDGET-AMEND-001.md"),
            "- **Crate:** `fmine-array`\n- **New planned:** 6200\n- **Approval:** approved by Owner, 2026-11-02\n",
        )
        .expect("w");
        let ok = check_amend(&root, "HEAD~1", true).expect("covered");
        assert_eq!(
            ok.warnings,
            ["BUDGET-AMEND-001: its approval is part of this change; confirm the owner wrote it"]
        );

        // Over the ceiling: adding fails with the record, a deletion passes.
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "approve"]);
        // Editing a record that was already approved at the base is not a new approval.
        fs::write(
            root.join(DELIVERY_DIR).join("BUDGET-AMEND-001.md"),
            "- **Crate:** `fmine-array`\n- **New planned:** 6200\n- **Scope:** more.rs\n\
             - **Approval:** approved by Owner, 2026-11-02\n",
        )
        .expect("w");
        let ok = check_amend(&root, "HEAD", true).expect("still covered");
        assert!(ok.warnings.is_empty(), "{:?}", ok.warnings);
        fs::write(src.join("more.rs"), lines(1000)).expect("w");
        let err = check_amend(&root, "HEAD", true).expect_err("over the ceiling");
        assert!(
            err[0].starts_with(
                "fmine-array: 7100 LOC is over its ceiling 7000 and this change adds 980"
            ),
            "{err:?}"
        );
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "overrun"]);
        fs::write(src.join("more.rs"), lines(990)).expect("w");
        let ok = check_amend(&root, "HEAD", true).expect("a deletion over the ceiling passes");
        assert!(
            ok.warnings[0].starts_with("fmine-array: 7090 LOC is over its ceiling 7000;"),
            "{:?}",
            ok.warnings
        );
    }

    /// The change is measured from the merge-base: a base branch that moved on is not the branch's deletion.
    #[test]
    fn test_change_is_measured_from_the_merge_base() {
        let (root, _c) = scratch("mergebase");
        let src = root.join("crates/fmine-array/src");
        fs::create_dir_all(&src).expect("src");
        fs::write(src.join("lib.rs"), lines(6100)).expect("w");
        let g = git_in(&root);
        g(&["init", "-q"]);
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "A"]);
        g(&["branch", "up"]);
        // This branch adds 20 lines; the base branch adds 300 elsewhere in the crate.
        fs::write(src.join("ours.rs"), lines(20)).expect("w");
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "B"]);
        g(&["checkout", "-q", "up"]);
        fs::write(src.join("theirs.rs"), lines(300)).expect("w");
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "U"]);
        g(&["checkout", "-q", "-"]);
        let change = measure(&root, "up").expect("measured").expect("repo");
        assert_eq!(
            change.net,
            BTreeMap::from([("fmine-array".to_string(), 20)])
        );
        assert!(check_amend(&root, "up", true).is_err());
    }

    #[test]
    fn test_net_loc_counts_non_blank_lines_and_a_non_repo_refuses_only_when_required() {
        let (root, _c) = scratch("net");
        let src = root.join("crates/fmine-io/src");
        fs::create_dir_all(&src).expect("src");
        // Ten code lines with two blank lines between, at the base too: blank lines never count.
        fs::write(src.join("lib.rs"), format!("{}\n\n{}", lines(5), lines(5))).expect("w");
        // Not yet a repository: nothing measured; CI mode refuses.
        assert_eq!(measure(&root, "HEAD").expect("no repo"), None);
        assert!(check_amend(&root, "HEAD", false).is_ok());
        let err = check_amend(&root, "HEAD", true).expect_err("CI must measure");
        assert!(
            err[0].ends_with(
                "is not a git work tree's top level; cannot measure the change since HEAD"
            ),
            "{err:?}"
        );
        let g = git_in(&root);
        g(&["init", "-q"]);
        // A non-ASCII file name, which git quotes unless asked for NUL-separated paths.
        fs::write(src.join("donn\u{e9}es.rs"), lines(3)).expect("w");
        fs::write(src.join("gone.rs"), lines(2)).expect("w");
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "ten"]);
        // lib.rs goes from ten code lines to four, with three blank lines between that do not count; an empty new
        // file and a README add nothing; the non-ASCII file gains two; gone.rs, deleted but still in the index,
        // loses two: net −6.
        fs::remove_file(src.join("gone.rs")).expect("rm");
        fs::write(
            src.join("lib.rs"),
            format!("{}\n\n\n{}", lines(2), lines(2)),
        )
        .expect("w");
        fs::write(src.join("extra.rs"), lines(0)).expect("w");
        fs::write(src.join("donn\u{e9}es.rs"), lines(5)).expect("w");
        fs::write(
            root.join("crates/fmine-io/README.md"),
            "docs do not count\n",
        )
        .expect("w");
        let net = measure(&root, "HEAD").expect("measured").expect("repo");
        assert_eq!(net.net, BTreeMap::from([("fmine-io".to_string(), -6)]));
        // A root inside a repository, not at its top level, is not measured.
        let inner = root.join("crates");
        assert_eq!(measure(&inner, "HEAD").expect("nested"), None);
    }
}
