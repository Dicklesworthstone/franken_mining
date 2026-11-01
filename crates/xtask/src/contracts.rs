//! The `CONTRACT.md` form (BD-327, §0.2): twelve sections, and Layer, Subsystem and LOC budget lines equal to
//! `registries/loc_budget.toml`.
//!
//! - **Check.** [`check_contract_text`] fails a contract whose level-2 headings are not exactly [`SECTIONS`], in
//!   order and numbered from 1, or whose section is empty, or whose section 1 `Layer`, `Subsystem` or
//!   `LOC budget` line differs from the crate's registry row, or names a subsystem outside [`SUBSYSTEMS`]. A
//!   `FMINE-REF-nnn` with no row in `registries/refusals.toml` is a warning until BD-328 closes. `xtask contracts`
//!   runs it over every `crates/*/CONTRACT.md`. `check_all` keeps `check_scope`'s older rule (the numbers
//!   `## 1. ` … `## 10. ` only) until BD-327.S2 rewrites the sixteen contracts and switches it to this check.
//! - **Generator.** `xtask contracts --write-budget` ([`write_budget`]) rewrites those three lines of every
//!   contract from the registry and changes no other byte, so the lines are never typed by hand. A second run
//!   changes nothing. A contract without exactly one of each line in section 1 is refused, and then no file is
//!   written.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::CrateBudgetRow;
use crate::refusals::{self, RefusalRow};

/// The required level-2 headings, in order: `## <n>. <name>`. 1–10 are §0.2's ten; 11 and 12 are BD-002's
/// "plus API, POs". Section 1 holds the Crate, Layer, Subsystem, LOC budget and Allowed Module Families lines.
pub const SECTIONS: [&str; 12] = [
    "Purpose",
    "Inputs",
    "Outputs",
    "Invariants",
    "Refusals",
    "Determinism class",
    "Evidence class of outputs",
    "Validity domain",
    "Oracles",
    "Known gaps",
    "Public API",
    "Proof obligations",
];

/// The §0.5 subsystems, the set `registries/loc_budget.toml` uses.
pub const SUBSYSTEMS: [&str; 18] = [
    "Bedrock",
    "Plumb",
    "Lodestar",
    "Corebox",
    "Strata",
    "Highwall",
    "Pushback",
    "Headframe",
    "Crucible",
    "Freeboard",
    "Muster",
    "Touchstone",
    "Bullion",
    "Hallmark",
    "Helm",
    "Portal",
    "Adit",
    "Testpit",
];

/// The one crate with no registry row: a dev-only tool, outside §28.3's budget.
pub const UNBUDGETED_CRATE: &str = "xtask";

/// Its `LOC budget` value.
pub const UNBUDGETED: &str = "not budgeted (dev-only tool; no row in registries/loc_budget.toml)";

const LAYER: &str = "Layer";
const SUBSYSTEM: &str = "Subsystem";
const LOC_BUDGET: &str = "LOC budget";

/// What [`check_contract_text`] found, by rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Findings {
    /// Heading names, order and empty sections.
    pub headings: Vec<String>,
    pub layer: Vec<String>,
    pub subsystem: Vec<String>,
    pub budget: Vec<String>,
    /// `FMINE-REF-nnn` codes with no registry row, sorted, once each.
    pub unregistered: Vec<String>,
    /// One per occurrence of an unregistered code.
    pub warnings: Vec<String>,
}

impl Findings {
    /// Every error, headings first.
    #[must_use]
    pub fn errors(&self) -> Vec<String> {
        [&self.headings, &self.layer, &self.subsystem, &self.budget]
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// `{"crate","headings_ok","budget_ok","layer_ok","subsystem_ok","unregistered_codes":[…]}`.
    #[must_use]
    pub fn log_line(&self, crate_name: &str) -> String {
        let codes: Vec<String> = self
            .unregistered
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect();
        format!(
            "{{\"crate\":\"{crate_name}\",\"headings_ok\":{},\"budget_ok\":{},\"layer_ok\":{},\
             \"subsystem_ok\":{},\"unregistered_codes\":[{}]}}",
            self.headings.is_empty(),
            self.budget.is_empty(),
            self.layer.is_empty(),
            self.subsystem.is_empty(),
            codes.join(",")
        )
    }
}

/// `(errors, warnings)` of one contract. `budget` is the crate's registry row; it is `None` only for
/// [`UNBUDGETED_CRATE`], whose `LOC budget` line must read [`UNBUDGETED`].
#[must_use]
pub fn check_contract_text(
    crate_name: &str,
    contract: &str,
    budget: Option<&CrateBudgetRow>,
    refusal_codes: &BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    let f = findings(crate_name, contract, budget, refusal_codes);
    (f.errors(), f.warnings)
}

/// [`check_contract_text`], by rule.
#[must_use]
pub fn findings(
    crate_name: &str,
    contract: &str,
    budget: Option<&CrateBudgetRow>,
    refusal_codes: &BTreeSet<String>,
) -> Findings {
    let path = format!("crates/{crate_name}/CONTRACT.md");
    let lines: Vec<&str> = contract.lines().collect();
    let heads = headings(&lines);
    let mut f = Findings {
        headings: heading_errors(&path, &lines, &heads),
        ..Findings::default()
    };

    let fields: Vec<(usize, &str, &str)> = section_one(&heads, lines.len())
        .filter_map(|i| {
            let (key, value) = field(lines.get(i)?)?;
            Some((i, key, value))
        })
        .collect();
    let found = |key: &str| lookup(&path, &fields, key);
    let unbudgeted = crate_name == UNBUDGETED_CRATE;
    match (found(LAYER), budget) {
        (Ok((at, value)), Some(row)) if value != row.layer => f.layer.push(format!(
            "{path}:{at}: Layer is `{value}`; registries/loc_budget.toml has `{}` (xtask contracts --write-budget)",
            row.layer
        )),
        (Err(e), _) => f.layer.push(e),
        _ => {}
    }
    match found(SUBSYSTEM) {
        Ok((at, value)) => {
            if !SUBSYSTEMS.contains(&value) {
                f.subsystem.push(format!(
                    "{path}:{at}: Subsystem `{value}` is not a §0.5 subsystem ({})",
                    SUBSYSTEMS.join(", ")
                ));
            }
            if let Some(row) = budget
                && value != row.subsystem
            {
                f.subsystem.push(format!(
                    "{path}:{at}: Subsystem is `{value}`; registries/loc_budget.toml has `{}` (xtask contracts \
                     --write-budget)",
                    row.subsystem
                ));
            }
        }
        Err(e) => f.subsystem.push(e),
    }
    let want = match budget {
        Some(row) => Some(budget_text(row)),
        None if unbudgeted => Some(UNBUDGETED.to_string()),
        None => {
            f.budget.push(format!(
                "{path}: `{crate_name}` has no row in registries/loc_budget.toml"
            ));
            None
        }
    };
    match (found(LOC_BUDGET), want) {
        (Ok((at, value)), Some(want)) if value != want => f.budget.push(format!(
            "{path}:{at}: LOC budget is `{value}`; it must read `{want}` (xtask contracts --write-budget)"
        )),
        (Err(e), _) => f.budget.push(e),
        _ => {}
    }

    let mut unregistered = BTreeSet::new();
    for (line, digits) in refusals::code_tokens(contract) {
        let code = format!("FMINE-REF-{digits}");
        if !refusal_codes.contains(&code) {
            f.warnings.push(format!(
                "{path}:{line}: `{code}` is not in registries/refusals.toml (a warning until BD-328 closes, \
                 then an error)"
            ));
            unregistered.insert(code);
        }
    }
    f.unregistered = unregistered.into_iter().collect();
    f
}

/// `6,000 planned / 7,000 ceiling`.
#[must_use]
pub fn budget_text(row: &CrateBudgetRow) -> String {
    format!(
        "{} planned / {} ceiling",
        thousands(row.planned_loc),
        thousands(row.ceiling_loc)
    )
}

/// `n` with a comma between each group of three digits.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// The level-2 headings outside fenced code: `(0-based line, heading)`.
fn headings<'a>(lines: &[&'a str]) -> Vec<(usize, &'a str)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with("## ") {
            out.push((i, line.trim_end()));
        }
    }
    out
}

/// The twelve headings, exact, once each and in order; and no empty section.
fn heading_errors(path: &str, lines: &[&str], heads: &[(usize, &str)]) -> Vec<String> {
    let mut errors = Vec::new();
    let wanted: Vec<String> = SECTIONS
        .iter()
        .enumerate()
        .map(|(i, name)| format!("## {}. {name}", i + 1))
        .collect();
    let mut misnamed = BTreeSet::new();
    for &(at, head) in heads {
        if wanted.iter().any(|w| w == head) {
            continue;
        }
        match wanted
            .iter()
            .enumerate()
            .find(|(i, _)| head.starts_with(&format!("## {}. ", i + 1)))
        {
            Some((i, w)) => {
                misnamed.insert(i);
                errors.push(format!(
                    "{path}:{}: expected `{w}`, found `{head}`",
                    at + 1
                ));
            }
            None => errors.push(format!(
                "{path}:{}: `{head}` is not one of the twelve sections (docs/templates/CONTRACT.md)",
                at + 1
            )),
        }
    }
    let mut order: Vec<(usize, usize)> = Vec::new();
    for (i, w) in wanted.iter().enumerate() {
        let hits: Vec<usize> = heads
            .iter()
            .filter(|(_, h)| h == w)
            .map(|&(at, _)| at)
            .collect();
        match hits.as_slice() {
            [] if !misnamed.contains(&i) => errors.push(format!("{path}: missing `{w}`")),
            [] => {}
            [at] => order.push((*at, i)),
            [_, again, ..] => errors.push(format!("{path}:{}: `{w}` again", again + 1)),
        }
    }
    order.sort_unstable();
    for pair in order.windows(2) {
        if let [(at_a, a), (at_b, b)] = pair
            && a > b
        {
            errors.push(format!(
                "{path}:{}: `{}` comes before `{}` (line {})",
                at_a + 1,
                wanted.get(*a).map_or("", String::as_str),
                wanted.get(*b).map_or("", String::as_str),
                at_b + 1
            ));
        }
    }
    for (k, &(at, head)) in heads.iter().enumerate() {
        let end = heads.get(k + 1).map_or(lines.len(), |h| h.0);
        let body = lines.get(at + 1..end).unwrap_or(&[]);
        if body.iter().all(|l| l.trim().is_empty()) {
            errors.push(format!("{path}:{}: `{head}` is empty", at + 1));
        }
    }
    errors
}

/// The 0-based lines of section 1: after the first level-2 heading, up to the next. Empty with no heading.
fn section_one(heads: &[(usize, &str)], len: usize) -> std::ops::Range<usize> {
    match heads {
        [] => 0..0,
        [(at, _)] => at + 1..len,
        [(at, _), (next, _), ..] => at + 1..*next,
    }
}

/// `- **Key:** value` as `(key, value)`.
fn field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.strip_prefix("- **")?.split_once(":**")?;
    Some((key, value.trim()))
}

/// The one section 1 line whose key is `key`: `(1-based line, value)`. A key in another case, a missing line
/// and a repeated one are errors.
fn lookup<'a>(
    path: &str,
    fields: &[(usize, &str, &'a str)],
    key: &str,
) -> Result<(usize, &'a str), String> {
    let hits: Vec<&(usize, &str, &str)> = fields
        .iter()
        .filter(|(_, k, _)| k.eq_ignore_ascii_case(key))
        .collect();
    match hits.as_slice() {
        [] => Err(format!("{path}: section 1 has no `- **{key}:**` line")),
        [(at, k, value)] if *k == key => Ok((at + 1, *value)),
        [(at, k, _)] => Err(format!("{path}:{}: `{k}` must be spelled `{key}`", at + 1)),
        [_, (at, ..), ..] => Err(format!(
            "{path}:{}: a second `{key}` line in section 1",
            at + 1
        )),
    }
}

/// `contract` with its section 1 Layer, Subsystem and LOC budget lines rewritten from `budget` and nothing else
/// changed. For [`UNBUDGETED_CRATE`] (`budget` `None`) only the LOC budget line is written. Refused when the
/// crate has no row, or section 1 lacks exactly one line for a key (matched in any case).
pub fn rewrite_budget_lines(
    crate_name: &str,
    contract: &str,
    budget: Option<&CrateBudgetRow>,
) -> Result<String, String> {
    let path = format!("crates/{crate_name}/CONTRACT.md");
    let wanted = match budget {
        Some(row) => vec![
            (LAYER, row.layer.clone()),
            (SUBSYSTEM, row.subsystem.clone()),
            (LOC_BUDGET, budget_text(row)),
        ],
        None if crate_name == UNBUDGETED_CRATE => vec![(LOC_BUDGET, UNBUDGETED.to_string())],
        None => {
            return Err(format!(
                "{path}: `{crate_name}` has no row in registries/loc_budget.toml"
            ));
        }
    };
    let bare: Vec<&str> = contract.lines().collect();
    let heads = headings(&bare);
    if heads.is_empty() {
        return Err(format!("{path}: no level-2 heading, so no section 1"));
    }
    let mut lines: Vec<String> = contract.split_inclusive('\n').map(str::to_string).collect();
    for (key, value) in wanted {
        let hits: Vec<usize> = section_one(&heads, bare.len())
            .filter(|&i| {
                bare.get(i)
                    .and_then(|l| field(l))
                    .is_some_and(|(k, _)| k.eq_ignore_ascii_case(key))
            })
            .collect();
        let [i] = hits.as_slice() else {
            return Err(format!(
                "{path}: section 1 needs exactly one `{key}` line, found {}",
                hits.len()
            ));
        };
        if let Some(line) = lines.get_mut(*i) {
            let ending = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            *line = format!("- **{key}:** {value}{ending}");
        }
    }
    Ok(lines.concat())
}

/// `(crate directory name, its CONTRACT.md)` for every `crates/*/CONTRACT.md`, by name.
fn contract_paths(root: &Path) -> Result<Vec<(String, PathBuf)>, Vec<String>> {
    let crates = root.join("crates");
    let dirs = fs::read_dir(&crates)
        .map_err(|e| vec![format!("cannot read {}: {e}", crates.display())])?;
    let mut out: Vec<(String, PathBuf)> = dirs
        .flatten()
        .map(|entry| entry.path().join("CONTRACT.md"))
        .filter(|p| p.is_file())
        .filter_map(|p| {
            let name = p.parent()?.file_name()?.to_string_lossy().into_owned();
            Some((name, p))
        })
        .collect();
    out.sort();
    Ok(out)
}

fn read(path: &Path) -> Result<String, Vec<String>> {
    fs::read_to_string(path).map_err(|e| vec![format!("cannot read {}: {e}", path.display())])
}

/// What `xtask contracts` found across the workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContractsReport {
    /// One [`Findings::log_line`] per contract, then `{"contracts","errors","warnings"}`.
    pub lines: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// [`check_contract_text`] on every `crates/*/CONTRACT.md` at `root`, against the registry rows and the codes of
/// `registries/refusals.toml`. `Err` only when a registry or a contract cannot be read.
pub fn check_contracts(root: &Path) -> Result<ContractsReport, Vec<String>> {
    let rows = crate::check_loc_budget(root)?.rows;
    let codes: BTreeSet<String> =
        refusals::parse_registry(&read(&root.join(refusals::REGISTRY_PATH))?)
            .map_err(|e| vec![e])?
            .iter()
            .map(RefusalRow::code)
            .collect();
    let mut report = ContractsReport::default();
    let paths = contract_paths(root)?;
    for (name, path) in &paths {
        let row = rows.iter().find(|r| &r.name == name);
        let f = findings(name, &read(path)?, row, &codes);
        report.lines.push(f.log_line(name));
        report.errors.extend(f.errors());
        report.warnings.extend(f.warnings);
    }
    report.lines.push(format!(
        "{{\"contracts\":{},\"errors\":{},\"warnings\":{}}}",
        paths.len(),
        report.errors.len(),
        report.warnings.len()
    ));
    Ok(report)
}

/// `xtask contracts --write-budget`: [`rewrite_budget_lines`] on every `crates/*/CONTRACT.md` at `root`, writing
/// only the files that change. Returns `{"crate","changed"}` per contract and `{"contracts","changed"}`. When any
/// contract is refused, no file is written.
pub fn write_budget(root: &Path) -> Result<Vec<String>, Vec<String>> {
    let rows = crate::check_loc_budget(root)?.rows;
    let mut plans = Vec::new();
    let mut errors = Vec::new();
    for (name, path) in contract_paths(root)? {
        let text = read(&path)?;
        match rewrite_budget_lines(&name, &text, rows.iter().find(|r| r.name == name)) {
            Ok(new) => plans.push((name, path, new != text, new)),
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut lines = Vec::new();
    for (name, path, changed, new) in &plans {
        if *changed {
            fs::write(path, new)
                .map_err(|e| vec![format!("cannot write {}: {e}", path.display())])?;
        }
        lines.push(format!("{{\"crate\":\"{name}\",\"changed\":{changed}}}"));
    }
    lines.push(format!(
        "{{\"contracts\":{},\"changed\":{}}}",
        plans.len(),
        plans.iter().filter(|p| p.2).count()
    ));
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(tag: &str) -> (PathBuf, Cleanup) {
        let base =
            std::env::temp_dir().join(format!("fmine-contracts-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("scratch");
        (base.clone(), Cleanup(base))
    }

    /// fmine-cli's row in `registries/loc_budget.toml`.
    fn cli_row() -> CrateBudgetRow {
        CrateBudgetRow {
            name: "fmine-cli".to_string(),
            layer: "U".to_string(),
            subsystem: "Adit".to_string(),
            planned_loc: 26_000,
            ceiling_loc: 30_000,
            actual_loc: 0,
            warn_80_pct: false,
            over_planned: false,
            over_ceiling: false,
        }
    }

    fn codes() -> BTreeSet<String> {
        ["FMINE-REF-101", "FMINE-REF-203"]
            .map(String::from)
            .into_iter()
            .collect()
    }

    /// A contract in the template's form; `budget_line` is section 1's LOC budget line.
    fn contract(budget_line: &str) -> String {
        let mut out = format!(
            "# CONTRACT.md — `fmine-cli`\n\n## 1. Purpose\nThe CLI.\n\n- **Crate:** `fmine-cli`\n\
             - **Layer:** U\n- **Subsystem:** Adit\n{budget_line}\n- **Allowed Module Families:** `[\"cli\"]`\n"
        );
        for (i, name) in SECTIONS.iter().enumerate().skip(1) {
            out.push_str(&format!(
                "\n## {}. {name}\nWhat the code does for {name}.\n",
                i + 1
            ));
        }
        out
    }

    fn good() -> String {
        contract("- **LOC budget:** 26,000 planned / 30,000 ceiling")
    }

    fn check(text: &str) -> (Vec<String>, Vec<String>) {
        check_contract_text("fmine-cli", text, Some(&cli_row()), &codes())
    }

    #[test]
    fn test_template_form_passes() {
        assert_eq!(check(&good()), (vec![], vec![]));
        let f = findings("fmine-cli", &good(), Some(&cli_row()), &codes());
        assert_eq!(
            f.log_line("fmine-cli"),
            "{\"crate\":\"fmine-cli\",\"headings_ok\":true,\"budget_ok\":true,\"layer_ok\":true,\
             \"subsystem_ok\":true,\"unregistered_codes\":[]}"
        );
        // A `## ` line in fenced code is not a heading.
        let fenced = good().replace(
            "What the code does for Inputs.\n",
            "What the code does for Inputs.\n```text\n## not a heading\n```\n",
        );
        assert_eq!(check(&fenced), (vec![], vec![]));
        // A `###` subsection, and a field-shaped line outside section 1, are body text.
        let body = good().replace(
            "What the code does for Inputs.\n",
            "What the code does for Inputs.\n### Files\n- **Layer:** not section 1\n",
        );
        assert_eq!(check(&body), (vec![], vec![]));
    }

    #[test]
    fn test_contract_misnamed_heading_fails() {
        let text = good().replace("## 1. Purpose", "## 1. Charter & Layer");
        let (errors, _) = check(&text);
        assert_eq!(
            errors,
            vec![
                "crates/fmine-cli/CONTRACT.md:3: expected `## 1. Purpose`, found `## 1. Charter & Layer`"
            ]
        );
        // A name with a suffix is not the name, and an unnumbered or extra heading is not a section.
        let suffixed = good().replace("## 6. Determinism class", "## 6. Determinism class (§29.2)");
        assert_eq!(check(&suffixed).0.len(), 1, "{:?}", check(&suffixed));
        let extra = format!("{}\n## Appendix\nMore.\n", good());
        let (errors, _) = check(&extra);
        assert!(
            errors.len() == 1
                && errors[0].contains("`## Appendix` is not one of the twelve sections"),
            "{errors:?}"
        );
    }

    #[test]
    fn test_contract_heading_out_of_order_fails() {
        let text = good()
            .replace("## 8. Validity domain", "## @8")
            .replace("## 9. Oracles", "## 8. Validity domain")
            .replace("## @8", "## 9. Oracles");
        let (errors, _) = check(&text);
        assert_eq!(
            errors,
            vec![
                "crates/fmine-cli/CONTRACT.md:30: `## 9. Oracles` comes before `## 8. Validity domain` (line 33)"
            ]
        );
        let repeated = format!("{}\n## 4. Invariants\nAgain.\n", good());
        let (errors, _) = check(&repeated);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("`## 4. Invariants` again")),
            "{errors:?}"
        );
    }

    #[test]
    fn test_contract_empty_section_fails() {
        let text = good().replace("What the code does for Invariants.\n", "\n   \n");
        let (errors, _) = check(&text);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("`## 4. Invariants` is empty"),
            "{errors:?}"
        );
        // The last section, running to the end of the file, too.
        let text = good().replace("What the code does for Proof obligations.\n", "");
        let (errors, _) = check(&text);
        assert!(
            errors.len() == 1 && errors[0].contains("`## 12. Proof obligations` is empty"),
            "{errors:?}"
        );
    }

    /// BD-327 criterion 4, at function level: a contract missing `## 8. Validity domain` fails.
    #[test]
    fn test_contract_missing_validity_domain_fails() {
        let text = good().replace(
            "\n## 8. Validity domain\nWhat the code does for Validity domain.\n",
            "",
        );
        let (errors, _) = check(&text);
        assert_eq!(
            errors,
            vec!["crates/fmine-cli/CONTRACT.md: missing `## 8. Validity domain`"]
        );
    }

    /// BD-327 criterion 4, at function level: fmine-cli's old `14k planned / 16k ceiling` fails against the row's
    /// 26,000/30,000, in the old form and with separators; so does a layer the row does not have.
    #[test]
    fn test_contract_budget_mismatch_fails() {
        for line in [
            "- **LOC budget:** 14k planned / 16k ceiling",
            "- **LOC budget:** 14,000 planned / 16,000 ceiling",
            "- **LOC budget:** 26000 planned / 30000 ceiling",
        ] {
            let (errors, _) = check(&contract(line));
            assert!(
                errors.len() == 1
                    && errors[0].contains("it must read `26,000 planned / 30,000 ceiling`"),
                "{line}: {errors:?}"
            );
        }
        let old = contract(
            "- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)",
        );
        let (errors, _) = check(&old);
        assert!(
            errors.len() == 1 && errors[0].contains("`LOC Budget` must be spelled `LOC budget`"),
            "{errors:?}"
        );
        let (errors, _) =
            check(&good().replace("- **Layer:** U", "- **Layer:** `U` (User Surfaces & CLI)"));
        assert!(
            errors.len() == 1 && errors[0].contains("registries/loc_budget.toml has `U`"),
            "{errors:?}"
        );
        let doubled = good().replace("- **Layer:** U\n", "- **Layer:** U\n- **Layer:** U\n");
        assert!(
            check(&doubled).0[0].contains("a second `Layer` line"),
            "{:?}",
            check(&doubled)
        );
        let (errors, _) = check(&good().replace("- **Layer:** U\n", ""));
        assert_eq!(
            errors,
            vec!["crates/fmine-cli/CONTRACT.md: section 1 has no `- **Layer:**` line"]
        );
    }

    #[test]
    fn test_contract_subsystem_not_in_section_0_5_fails() {
        let (errors, _) =
            check(&good().replace("- **Subsystem:** Adit", "- **Subsystem:** Enterprise"));
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors[0].contains("Subsystem `Enterprise` is not a §0.5 subsystem"),
            "{errors:?}"
        );
        assert!(
            errors[1].contains("registries/loc_budget.toml has `Adit`"),
            "{errors:?}"
        );
        // A §0.5 name that is not the row's fails on the registry rule only.
        let (errors, _) =
            check(&good().replace("- **Subsystem:** Adit", "- **Subsystem:** Portal"));
        assert!(
            errors.len() == 1 && errors[0].contains("Subsystem is `Portal`"),
            "{errors:?}"
        );
    }

    #[test]
    fn test_contract_unregistered_refusal_warns() {
        let text = good().replace(
            "What the code does for Refusals.",
            "FMINE-REF-101 and FMINE-REF-109 on bad input; FMINE-REF-1xx in general.",
        );
        let (errors, warnings) = check(&text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("`FMINE-REF-109` is not in registries/refusals.toml"));
        let f = findings("fmine-cli", &text, Some(&cli_row()), &codes());
        assert_eq!(f.unregistered, vec!["FMINE-REF-109"]);
        assert!(
            f.log_line("fmine-cli")
                .ends_with("\"unregistered_codes\":[\"FMINE-REF-109\"]}")
        );
    }

    #[test]
    fn test_contract_xtask_unbudgeted_line_accepted() {
        let xtask = |line: &str| {
            let text = contract(line)
                .replace("fmine-cli", "xtask")
                .replace("- **Layer:** U", "- **Layer:** T")
                .replace("- **Subsystem:** Adit", "- **Subsystem:** Testpit");
            check_contract_text("xtask", &text, None, &codes()).0
        };
        assert_eq!(
            xtask(&format!("- **LOC budget:** {UNBUDGETED}")),
            Vec::<String>::new()
        );
        let errors = xtask("- **LOC budget:** Dev-only verification binary (`GATE-LAYERS-00`)");
        assert!(
            errors.len() == 1 && errors[0].contains(&format!("it must read `{UNBUDGETED}`")),
            "{errors:?}"
        );
        // Any other crate needs its row.
        let (errors, _) = check_contract_text("fmine-cli", &good(), None, &codes());
        assert_eq!(
            errors,
            vec![
                "crates/fmine-cli/CONTRACT.md: `fmine-cli` has no row in registries/loc_budget.toml"
            ]
        );
    }

    #[test]
    fn test_thousands_separators() {
        for (n, s) in [
            (0, "0"),
            (999, "999"),
            (1_000, "1,000"),
            (26_000, "26,000"),
            (1_128_000, "1,128,000"),
        ] {
            assert_eq!(thousands(n), s);
        }
    }

    /// The constant is the registry's set: no §0.5 name misspelt here, none missing there.
    #[test]
    fn test_subsystems_are_the_registry_set() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = fs::read_to_string(root.join("registries/loc_budget.toml")).expect("registry");
        let used: BTreeSet<String> = text
            .lines()
            .filter_map(|l| crate::strip_kv_str(l.trim(), "subsystem"))
            .collect();
        let wanted: BTreeSet<String> = SUBSYSTEMS.map(String::from).into_iter().collect();
        assert_eq!(used, wanted);
        let rows = crate::check_loc_budget(&root).expect("rows").rows;
        assert!(
            rows.iter()
                .all(|r| SUBSYSTEMS.contains(&r.subsystem.as_str()))
        );
    }

    /// `--write-budget` on a fixture directory: the old fmine-cli lines become the row's, xtask's budget line
    /// becomes the unbudgeted one, no other byte moves, and a second run is byte-identical. A contract with a
    /// doubled line is refused and then nothing is written.
    #[test]
    fn test_write_budget_is_idempotent() {
        let (root, _cleanup) = scratch("write");
        fs::create_dir_all(root.join("registries")).expect("dir");
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        fs::copy(
            workspace.join("registries/loc_budget.toml"),
            root.join("registries/loc_budget.toml"),
        )
        .expect("registry");
        let old_cli = "# CONTRACT.md — `fmine-cli`\n\n## 1. Charter & Layer\n- **Crate:** `fmine-cli`\n\
                       - **Layer:** `U` (User Surfaces & CLI)\n- **Subsystem:** `Surfaces`\r\n\
                       - **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)\n\
                       - **Allowed Module Families:** `[\"cli\"]`\r\n\n## 2. Mathematical\nLayer: U\n\
                       - **Layer:** outside section 1\n";
        let old_xtask = "## 1. Charter & Layer\n- **Layer:** `T` (Build/Verification tooling)\n\
                         - **Subsystem:** `Testpit`\n- **LOC Budget:** Dev-only verification binary\n";
        for (name, text) in [("fmine-cli", old_cli), ("xtask", old_xtask)] {
            fs::create_dir_all(root.join("crates").join(name)).expect("dir");
            fs::write(root.join("crates").join(name).join("CONTRACT.md"), text).expect("write");
        }
        let cli = root.join("crates/fmine-cli/CONTRACT.md");
        let xtask = root.join("crates/xtask/CONTRACT.md");

        let first = write_budget(&root).expect("first run");
        assert_eq!(
            first,
            vec![
                "{\"crate\":\"fmine-cli\",\"changed\":true}",
                "{\"crate\":\"xtask\",\"changed\":true}",
                "{\"contracts\":2,\"changed\":2}"
            ]
        );
        let cli_text = fs::read_to_string(&cli).expect("read");
        assert_eq!(
            cli_text,
            old_cli
                .replace("- **Layer:** `U` (User Surfaces & CLI)", "- **Layer:** U")
                .replace("- **Subsystem:** `Surfaces`", "- **Subsystem:** Adit")
                .replace(
                    "- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)",
                    "- **LOC budget:** 26,000 planned / 30,000 ceiling"
                )
        );
        let f = findings("fmine-cli", &cli_text, Some(&cli_row()), &codes());
        assert!(
            f.layer.is_empty() && f.subsystem.is_empty() && f.budget.is_empty(),
            "{f:?}"
        );
        assert_eq!(
            fs::read_to_string(&xtask).expect("read"),
            old_xtask.replace(
                "- **LOC Budget:** Dev-only verification binary",
                &format!("- **LOC budget:** {UNBUDGETED}")
            )
        );

        let (cli_bytes, xtask_bytes) = (
            fs::read(&cli).expect("cli"),
            fs::read(&xtask).expect("xtask"),
        );
        let second = write_budget(&root).expect("second run");
        assert_eq!(
            second.last().map(String::as_str),
            Some("{\"contracts\":2,\"changed\":0}")
        );
        assert_eq!(fs::read(&cli).expect("cli"), cli_bytes);
        assert_eq!(fs::read(&xtask).expect("xtask"), xtask_bytes);

        // Refused: xtask's section 1 has two LOC budget lines; fmine-cli, though fine, is not written either.
        fs::write(&cli, old_cli).expect("reset");
        let doubled = format!("{old_xtask}- **loc budget:** again\n");
        fs::write(&xtask, &doubled).expect("double");
        let errors = write_budget(&root).expect_err("refused");
        assert_eq!(
            errors,
            vec![
                "crates/xtask/CONTRACT.md: section 1 needs exactly one `LOC budget` line, found 2"
            ]
        );
        assert_eq!(fs::read_to_string(&cli).expect("cli"), old_cli);
        assert_eq!(fs::read_to_string(&xtask).expect("xtask"), doubled);

        // A crate with no row is refused, whatever its text.
        let text = rewrite_budget_lines("fmine-nope", old_cli, None).expect_err("no row");
        assert!(text.contains("`fmine-nope` has no row"), "{text}");
    }
}
