//! FrankenMining `xtask` governance and architectural verification library (`GATE-LAYERS-00`, BD-003..BD-015).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub mod forecast;
pub mod int_as;
pub mod lifts;
pub mod refusals;

/// Summary row for a single crate's LOC budget (`BD-003`, `BD-012`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateBudgetRow {
    /// Crate name (`fmine-*`).
    pub name: String,
    /// Architectural layer (`S0`, `S1`, `K0`..`K4`, `E`, `V`, `U`, `T`, `GATED`).
    pub layer: String,
    /// Planned LOC budget.
    pub planned_loc: usize,
    /// Hard 1.15x ceiling LOC budget.
    pub ceiling_loc: usize,
    /// Actual measured Rust LOC across `src/`, `tests/`, `benches/`.
    pub actual_loc: usize,
    /// True if `actual_loc >= 80%` of `planned_loc`.
    pub warn_80_pct: bool,
    /// True if `actual_loc > planned_loc`.
    pub over_planned: bool,
    /// True if `actual_loc > ceiling_loc`.
    pub over_ceiling: bool,
}

/// Aggregate LOC report across the workspace (`BD-003`, `BD-012`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocReport {
    /// All 59 registered crate rows.
    pub rows: Vec<CrateBudgetRow>,
    /// Total planned LOC across all registered crates.
    pub total_planned: usize,
    /// Total ceiling LOC across all registered crates.
    pub total_ceiling: usize,
    /// Total actual Rust LOC across all existing workspace crates.
    pub total_actual: usize,
}

/// Parse `registries/loc_budget.toml` and count actual Rust lines (`BD-003`, `BD-012`).
pub fn check_loc_budget(root: &Path) -> Result<LocReport, Vec<String>> {
    let budget_path = root.join("registries/loc_budget.toml");
    let content = fs::read_to_string(&budget_path)
        .map_err(|e| vec![format!("failed to read {}: {e}", budget_path.display())])?;

    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut cur_name = String::new();
    let mut cur_layer = String::new();
    let mut cur_planned = 0usize;
    let mut cur_ceiling = 0usize;
    let mut in_crate = false;

    let flush = |name: &str,
                 layer: &str,
                 planned: usize,
                 ceiling: usize,
                 rows: &mut Vec<CrateBudgetRow>,
                 errors: &mut Vec<String>| {
        if name.is_empty() {
            return;
        }
        let crate_dir = root.join("crates").join(name);
        let actual_loc = if crate_dir.exists() {
            count_rust_loc(&crate_dir)
        } else {
            0
        };
        let warn_80_pct = planned > 0 && actual_loc * 100 >= planned * 80;
        let over_planned = actual_loc > planned;
        let over_ceiling = actual_loc > ceiling;
        if over_ceiling {
            errors.push(format!(
                "LOC ceiling overrun in {name}: actual {actual_loc} > ceiling {ceiling}"
            ));
        }
        rows.push(CrateBudgetRow {
            name: name.to_string(),
            layer: layer.to_string(),
            planned_loc: planned,
            ceiling_loc: ceiling,
            actual_loc,
            warn_80_pct,
            over_planned,
            over_ceiling,
        });
    };

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line == "[[crate]]" {
            if in_crate {
                flush(
                    &cur_name,
                    &cur_layer,
                    cur_planned,
                    cur_ceiling,
                    &mut rows,
                    &mut errors,
                );
            }
            cur_name.clear();
            cur_layer.clear();
            cur_planned = 0;
            cur_ceiling = 0;
            in_crate = true;
        } else if in_crate {
            if let Some(val) = strip_kv_str(line, "name") {
                cur_name = val;
            } else if let Some(val) = strip_kv_str(line, "layer") {
                cur_layer = val;
            } else if let Some(val) = strip_kv_usize(line, "planned_loc") {
                cur_planned = val;
            } else if let Some(val) = strip_kv_usize(line, "ceiling_loc") {
                cur_ceiling = val;
            }
        }
    }
    if in_crate {
        flush(
            &cur_name,
            &cur_layer,
            cur_planned,
            cur_ceiling,
            &mut rows,
            &mut errors,
        );
    }

    if rows.len() < 59 {
        errors.push(format!(
            "registries/loc_budget.toml must contain all 59 crates (§28.3), found {}",
            rows.len()
        ));
    }

    let total_planned: usize = rows.iter().map(|r| r.planned_loc).sum();
    let total_ceiling: usize = rows.iter().map(|r| r.ceiling_loc).sum();
    let total_actual: usize = rows.iter().map(|r| r.actual_loc).sum();

    if total_planned != 1_128_000 {
        errors.push(format!(
            "total_planned in loc_budget.toml must equal 1,128,000 (§28.3), found {total_planned}"
        ));
    }
    if total_ceiling != 1_298_000 {
        errors.push(format!(
            "total_ceiling in loc_budget.toml must equal 1,298,000 (§28.3), found {total_ceiling}"
        ));
    }

    if errors.is_empty() {
        Ok(LocReport {
            rows,
            total_planned,
            total_ceiling,
            total_actual,
        })
    } else {
        Err(errors)
    }
}

/// Validate downward-only layer dependencies (`L-1`, `BD-004`).
pub fn check_layers(root: &Path) -> Result<(), Vec<String>> {
    let report = check_loc_budget(root)?;
    let crate_layers: BTreeMap<String, String> =
        report.rows.into_iter().map(|r| (r.name, r.layer)).collect();

    let topo_path = root.join("registries/workspace_topology.toml");
    let topo_text = fs::read_to_string(&topo_path)
        .map_err(|e| vec![format!("failed to read {}: {e}", topo_path.display())])?;

    let mut ranks = BTreeMap::new();
    let mut allowed_same_layer = BTreeSet::new();
    let mut in_ranks = false;
    let mut in_edge = false;
    let mut edge_from = String::new();
    let mut edge_to = String::new();

    for raw_line in topo_text.lines() {
        let line = raw_line.trim();
        if line == "[layer_ranks]" {
            in_ranks = true;
            in_edge = false;
            continue;
        } else if line == "[[allowed_same_layer_edge]]" {
            if in_edge && !edge_from.is_empty() && !edge_to.is_empty() {
                allowed_same_layer.insert((edge_from.clone(), edge_to.clone()));
            }
            edge_from.clear();
            edge_to.clear();
            in_ranks = false;
            in_edge = true;
            continue;
        } else if line.starts_with('[') {
            in_ranks = false;
            in_edge = false;
        }

        if in_ranks {
            if let Some((k, v)) = line.split_once('=')
                && let Ok(r) = v.trim().parse::<usize>()
            {
                ranks.insert(k.trim().to_string(), r);
            }
        } else if in_edge {
            if let Some(val) = strip_kv_str(line, "from") {
                edge_from = val;
            } else if let Some(val) = strip_kv_str(line, "to") {
                edge_to = val;
            }
        }
    }
    if in_edge && !edge_from.is_empty() && !edge_to.is_empty() {
        allowed_same_layer.insert((edge_from, edge_to));
    }

    let mut errors = Vec::new();
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, deps, _dev_deps) = parse_cargo_manifest(&member_toml)?;
        if pkg_name == "xtask" {
            continue;
        }
        let Some(from_layer) = crate_layers.get(&pkg_name) else {
            errors.push(format!(
                "crate {pkg_name} not registered in loc_budget.toml"
            ));
            continue;
        };
        let Some(&from_rank) = ranks.get(from_layer) else {
            errors.push(format!("unknown layer {from_layer} for crate {pkg_name}"));
            continue;
        };
        for dep in deps {
            if !dep.starts_with("fmine-") {
                continue;
            }
            let Some(to_layer) = crate_layers.get(&dep) else {
                errors.push(format!(
                    "dependency {dep} of {pkg_name} not registered in loc_budget.toml"
                ));
                continue;
            };
            let Some(&to_rank) = ranks.get(to_layer) else {
                errors.push(format!("unknown layer {to_layer} for dependency {dep}"));
                continue;
            };
            validate_layer_edge(
                &pkg_name,
                from_layer,
                from_rank,
                &dep,
                to_layer,
                to_rank,
                &allowed_same_layer,
            )
            .err()
            .into_iter()
            .for_each(|e| errors.push(e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate a single dependency edge `(from -> to)` against layer ranks and allowed intra-layer DAG edges (`BD-004`).
pub fn validate_layer_edge(
    from_crate: &str,
    from_layer: &str,
    from_rank: usize,
    to_crate: &str,
    to_layer: &str,
    to_rank: usize,
    allowed_same_layer: &BTreeSet<(String, String)>,
) -> Result<(), String> {
    if from_rank < to_rank {
        return Err(format!(
            "upward layer dependency forbidden (L-1): {from_crate} ({from_layer}, rank {from_rank}) -> {to_crate} ({to_layer}, rank {to_rank})"
        ));
    }
    if from_rank == to_rank
        && from_layer != "T"
        && !allowed_same_layer.contains(&(from_crate.to_string(), to_crate.to_string()))
    {
        return Err(format!(
            "undeclared same-layer dependency (L-1): {from_crate} ({from_layer}) -> {to_crate} ({to_layer}); register in workspace_topology.toml"
        ));
    }
    Ok(())
}

/// Validate that no workspace crate pulls a banned dependency from `deny.toml` (`L-5`, `BD-005`)
/// and no production crate depends on a `T`-layer oracle crate (`GATE-ORACLE-01`, `BD-013`).
pub fn check_deps(root: &Path) -> Result<(), Vec<String>> {
    let deny_path = root.join("deny.toml");
    let deny_text = fs::read_to_string(&deny_path)
        .map_err(|e| vec![format!("failed to read {}: {e}", deny_path.display())])?;

    let banned =
        banned_crates(&deny_text).map_err(|e| vec![format!("{}: {e}", deny_path.display())])?;
    // An empty list would pass every crate, so a deny.toml this reader cannot parse fails instead.
    if banned.is_empty() {
        return Err(vec![format!(
            "{}: no banned crate read (expected `{{ name = \"...\" }}` rows in a multi-line `[bans] deny`)",
            deny_path.display()
        )]);
    }

    let oracle_crates: BTreeSet<&str> = [
        "fmine-oracles",
        "fmine-gen",
        "fmine-verify",
        "fmine-derive",
        "frankensympy",
        "fsym-core",
    ]
    .into_iter()
    .collect();

    let mut errors = Vec::new();
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, deps, dev_deps) = parse_cargo_manifest(&member_toml)?;
        for dep in deps.iter().chain(dev_deps.iter()) {
            if banned.contains(dep) {
                errors.push(format!(
                    "banned dependency {dep} in crate {pkg_name} (violates L-5 / deny.toml)"
                ));
            }
        }
        if pkg_name != "xtask" && !oracle_crates.contains(pkg_name.as_str()) {
            for dep in &deps {
                if oracle_crates.contains(dep.as_str()) {
                    errors.push(format!(
                        "production crate {pkg_name} depends on oracle/verification crate {dep} (violates GATE-ORACLE-01)"
                    ));
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Alias for `GATE-ORACLE-01` check (`BD-013`).
pub fn check_oracle(root: &Path) -> Result<(), Vec<String>> {
    check_deps(root)
}

/// Check capability rules (`L-3`, `BD-006`): ban `open_child_region` outside `fmine-runtime`,
/// and ban `std::fs`, `std::net`, `std::process`, `SystemTime::now` in `S0`, `K`, and `E` production `src/` files.
pub fn check_caps(root: &Path) -> Result<(), Vec<String>> {
    let report = check_loc_budget(root)?;
    let crate_layers: BTreeMap<String, String> =
        report.rows.into_iter().map(|r| (r.name, r.layer)).collect();

    let mut errors = Vec::new();
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, _, _) = parse_cargo_manifest(&member_toml)?;
        if pkg_name == "xtask" {
            continue;
        }
        let layer = crate_layers.get(&pkg_name).cloned().unwrap_or_default();
        let src_dir = member_toml.parent().unwrap_or(root).join("src");
        for rs_file in collect_rs_files(&src_dir) {
            let content = fs::read_to_string(&rs_file).unwrap_or_default();
            let prod_code = strip_cfg_test(&content);
            check_caps_in_source(&pkg_name, &layer, &rs_file, &prod_code)
                .err()
                .into_iter()
                .for_each(|e| errors.push(e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Inspect a single production source string for capability violations (`L-3`, `BD-006`).
pub fn check_caps_in_source(
    pkg_name: &str,
    layer: &str,
    path: &Path,
    prod_code: &str,
) -> Result<(), String> {
    if pkg_name != "fmine-runtime" && prod_code.contains("open_child_region") {
        return Err(format!(
            "{}: `open_child_region` is banned outside `fmine-runtime` (L-3, S3(a))",
            path.display()
        ));
    }
    let is_pure_layer =
        (layer == "S0" && pkg_name != "fmine-registry") || layer.starts_with('K') || layer == "E";
    if is_pure_layer {
        for forbidden in [
            "std::fs::",
            "std::net::",
            "std::process::Command",
            "SystemTime::now",
        ] {
            if prod_code.contains(forbidden) {
                return Err(format!(
                    "{}: forbidden capability `{forbidden}` in {pkg_name} ({layer}); K/E/S0 kernels must be pure (L-3)",
                    path.display()
                ));
            }
        }
    }
    if pkg_name != "fmine-runtime" && pkg_name != "fmine-lab" {
        for forbidden in DETACHED_OR_AMBIENT {
            if prod_code.contains(forbidden) {
                return Err(format!(
                    "{}: `{forbidden}` in {pkg_name}: no detached worker or ambient timer outside the runtime (criterion 9)",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

/// INT-AS-001's criterion 9 in our own code (BD-325): every layer, except the two crates that own these.
const DETACHED_OR_AMBIENT: [&str; 5] = [
    "thread::spawn",
    "thread::Builder",
    "thread::sleep",
    "Instant::now",
    "SystemTime::now",
];

/// Check that every workspace crate has a 10-section `CONTRACT.md` and every module `src/lib.rs` declares, at
/// any visibility and outside `#[cfg(test)]`, is listed in `Allowed Module Families` (`BD-002`, `BD-008`).
pub fn check_scope(root: &Path) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, _, _) = parse_cargo_manifest(&member_toml)?;
        let crate_dir = member_toml.parent().unwrap_or(root);
        let contract_path = crate_dir.join("CONTRACT.md");
        if !contract_path.exists() {
            errors.push(format!(
                "missing CONTRACT.md in crate {pkg_name} ({})",
                contract_path.display()
            ));
            continue;
        }
        let contract = fs::read_to_string(&contract_path).unwrap_or_default();
        for sec in 1..=10 {
            let heading = format!("## {sec}. ");
            if !contract.contains(&heading) {
                errors.push(format!(
                    "{}: missing required contract section `{heading}` (BD-002)",
                    contract_path.display()
                ));
            }
        }
        let allowed_mods = parse_allowed_modules(&contract);
        let lib_rs = crate_dir.join("src/lib.rs");
        if lib_rs.exists() {
            let lib_text = fs::read_to_string(&lib_rs).unwrap_or_default();
            for mod_name in declared_modules(&lib_text) {
                if !allowed_mods.contains(mod_name) {
                    errors.push(format!(
                        "undeclared module `{mod_name}` in {pkg_name}/src/lib.rs not in CONTRACT.md Allowed Module Families {:?} (BD-008)",
                        allowed_mods
                    ));
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The top-level modules `lib_text` declares, at any visibility (`mod`, `pub mod`, `pub(crate) mod`, …), in a
/// file or inline. A module under `#[cfg(test)]` is skipped; any other private module is reported like a public
/// one, so visibility is no way around the charter (BD-008).
fn declared_modules(lib_text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut cfg_test = false;
    for line in lib_text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        if t.starts_with("#[") {
            cfg_test |= t == "#[cfg(test)]";
            continue;
        }
        // Top-level items start in column 0; a `mod` nested in another module's body is not a crate module.
        if !cfg_test
            && !line.starts_with(char::is_whitespace)
            && let Some(name) = module_name(t)
        {
            out.push(name);
        }
        cfg_test = false;
    }
    out
}

/// The module name an item line declares (`pub(crate) mod foo {` gives `foo`), or `None`.
fn module_name(item: &str) -> Option<&str> {
    let rest = match item.strip_prefix("pub") {
        Some(v) if v.starts_with('(') => v.get(v.find(')')? + 1..)?.trim_start(),
        Some(v) if v.starts_with(' ') => v.trim_start(),
        Some(_) => return None,
        None => item,
    };
    let name = rest
        .strip_prefix("mod ")?
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    (!name.is_empty()).then_some(name)
}

/// Check `L-6` (`BD-009`): no bare `f64` physical quantity parameters (`tonnes: f64`, `grade: f64`, `ounces: f64`, `moisture: f64`)
/// in `K` or `E` public function signatures.
pub fn check_claims(root: &Path) -> Result<(), Vec<String>> {
    let report = check_loc_budget(root)?;
    let crate_layers: BTreeMap<String, String> =
        report.rows.into_iter().map(|r| (r.name, r.layer)).collect();

    let mut errors = Vec::new();
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, _, _) = parse_cargo_manifest(&member_toml)?;
        let layer = crate_layers.get(&pkg_name).cloned().unwrap_or_default();
        if !(layer.starts_with('K') || layer == "E") || pkg_name == "fmine-numerics" {
            continue;
        }
        let src_dir = member_toml.parent().unwrap_or(root).join("src");
        for rs_file in collect_rs_files(&src_dir) {
            let content = fs::read_to_string(&rs_file).unwrap_or_default();
            let prod_code = strip_cfg_test(&content);
            if let Err(e) = check_bare_quantity_signature(&rs_file, &prod_code) {
                errors.push(e);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check a source string for bare `f64` physical quantities in `pub fn` signatures (`L-6`, `BD-009`).
pub fn check_bare_quantity_signature(path: &Path, prod_code: &str) -> Result<(), String> {
    let banned_params = [
        "wet_tonnes: f64",
        "dry_tonnes: f64",
        "grade_gpt: f64",
        "moisture_pct: f64",
        "ounces_gold: f64",
    ];
    for line in prod_code.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ") {
            for pat in banned_params {
                if trimmed.contains(pat) {
                    return Err(format!(
                        "{}: bare physical quantity `{pat}` in K/E public signature `{trimmed}` (violates L-6; use Qty<D, B, R> or ExactQty)",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The determinism classes of plan §29.2, as a `//! Determinism: <class>` header names them.
pub const DET_CLASSES: [&str; 4] = [
    "Exact",
    "BitwisePortable",
    "DeterministicPerIsa",
    "ThreadVariant",
];

/// Std float methods that call the platform libm (§29.3), banned in `BitwisePortable` modules. `sqrt`, `mul_add`,
/// `abs`, `floor`, `ceil`, `round`, `min` and `max` are correctly rounded or exact, and allowed.
pub const BANNED_TRANSCENDENTALS: [&str; 21] = [
    "ln", "log2", "log10", "ln_1p", "exp", "exp2", "exp_m1", "sin", "cos", "tan", "asin", "acos",
    "atan", "atan2", "sinh", "cosh", "tanh", "powf", "powi", "cbrt", "hypot",
];

/// The one file allowed to implement transcendentals, by its path relative to the workspace root.
pub const DET_EXEMPT: &str = "crates/fmine-core/src/det.rs";

/// What `check-det` checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetReport {
    /// Production `.rs` files read (every crate except xtask).
    pub files: usize,
    /// Files whose header declares `BitwisePortable`.
    pub bitwise_files: usize,
}

impl DetReport {
    /// The one log line `check-det` and `check-all` print; a passing run has no missing headers or violations.
    #[must_use]
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"check-det\",\"files\":{},\"headers_missing\":0,\"violations\":0,\"bitwise_files\":{}}}",
            self.files, self.bitwise_files
        )
    }
}

/// Check `PO-NUM-2` (`BD-010`, bd-hp2j.1, plan §29.2–29.3):
/// - every production file of a crate whose layer starts with `K` or is `E` carries `//! Determinism: <class>`,
///   with a class from [`DET_CLASSES`];
/// - a `BitwisePortable` file calls none of [`BANNED_TRANSCENDENTALS`] as `.name(` or `f64::name(`, outside
///   comments and `#[cfg(test)]` code. Only [`DET_EXEMPT`] is exempt.
///
/// An unreadable file is an error.
pub fn check_det(root: &Path) -> Result<DetReport, Vec<String>> {
    let layers: BTreeMap<String, String> = check_loc_budget(root)?
        .rows
        .into_iter()
        .map(|r| (r.name, r.layer))
        .collect();
    let mut errors = Vec::new();
    let mut report = DetReport {
        files: 0,
        bitwise_files: 0,
    };
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, _, _) = parse_cargo_manifest(&member_toml)?;
        if pkg_name == "xtask" {
            continue;
        }
        let layer = layers.get(&pkg_name).map_or("", String::as_str);
        let src_dir = member_toml.parent().unwrap_or(root).join("src");
        check_det_dir(root, layer, &src_dir, &mut report, &mut errors);
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}

/// `check-det` over one crate's `src` directory.
fn check_det_dir(
    root: &Path,
    layer: &str,
    src_dir: &Path,
    report: &mut DetReport,
    errors: &mut Vec<String>,
) {
    for rs_file in collect_rs_files(src_dir) {
        let rel = rs_file.strip_prefix(root).unwrap_or(&rs_file);
        let content = match fs::read_to_string(&rs_file) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("{}: unreadable ({e})", rel.display()));
                continue;
            }
        };
        report.files += 1;
        match check_det_in_source(rel, layer, &strip_cfg_test(&content)) {
            Ok(Some("BitwisePortable")) => report.bitwise_files += 1,
            Ok(_) => {}
            Err(e) => errors.push(e),
        }
    }
}

/// The class a `//! Determinism: <class>` line declares (its first word), if the code has such a line.
fn det_class(prod_code: &str) -> Option<&str> {
    prod_code.lines().find_map(|l| {
        l.trim_start()
            .strip_prefix("//! Determinism:")
            .and_then(|rest| rest.split_whitespace().next())
    })
}

/// Every banned std transcendental call in `prod_code`, as `(line number, name)`, one entry per call: the method
/// form `.name(` and the path form `f64::name(`, with `//` comments removed first. The `(` must follow the name,
/// so `.exp(` never matches `.exp2(` or `.exp_m1(`.
#[must_use]
pub fn banned_transcendental_calls(prod_code: &str) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    for (i, line) in prod_code.lines().enumerate() {
        let code = line.find("//").map_or(line, |at| &line[..at]);
        for name in BANNED_TRANSCENDENTALS {
            for prefix in [".", "f64::"] {
                out.extend(
                    code.matches(&format!("{prefix}{name}("))
                        .map(|_| (i + 1, name)),
                );
            }
        }
    }
    out
}

/// Checks one file's production code (`path` relative to the workspace root, `layer` its crate's layer) and
/// returns its declared class, if any (`PO-NUM-2`, `BD-010`, bd-hp2j.1).
pub fn check_det_in_source<'a>(
    path: &Path,
    layer: &str,
    prod_code: &'a str,
) -> Result<Option<&'a str>, String> {
    let class = det_class(prod_code);
    match class {
        None if layer.starts_with('K') || layer == "E" => {
            return Err(format!(
                "{}: layer {layer} file has no `//! Determinism: <class>` header (plan §29.2; one of {})",
                path.display(),
                DET_CLASSES.join(", ")
            ));
        }
        Some(c) if !DET_CLASSES.contains(&c) => {
            return Err(format!(
                "{}: unknown determinism class `{c}` (plan §29.2; one of {})",
                path.display(),
                DET_CLASSES.join(", ")
            ));
        }
        _ => {}
    }
    if class == Some("BitwisePortable") && path != Path::new(DET_EXEMPT) {
        let calls = banned_transcendental_calls(prod_code);
        if !calls.is_empty() {
            let list: Vec<String> = calls
                .iter()
                .map(|(l, n)| format!("{n} at line {l}"))
                .collect();
            return Err(format!(
                "{}: {} banned std transcendental call(s) in a BitwisePortable module ({}; PO-NUM-2, use \
                 fmine_core::det)",
                path.display(),
                calls.len(),
                list.join(", ")
            ));
        }
    }
    Ok(class)
}

/// `registry-check` (bd-s9j3.1): every `registries/sources/*.toml` parses in the TOML subset and passes the
/// source-registry rules (unique files, paths and digests; lowercase 64-hex digests; `YYYYQn` quarters;
/// registered kinds, URL bases and families; reconstructions pointing at registered rows), and its `issuer`
/// matches its file name. A missing directory is an error: the public lane cannot run without it.
///
/// On success it returns one JSON log line per registry, in file-name order:
/// `{"stage":"registry","issuer","sources","subjects","reconstructions","label_checks","mine_stats_labels",
/// "documents","check":"pass"}`. `label_checks` counts the checks the run makes: a set shared by two
/// reconstructions counts once for each.
///
/// When `registries/transcriptions/<issuer>.toml` exists (git-ignored, so never in CI; bd-plx1), it must parse
/// against the issuer's registered documents and pass the two-reader check with no disagreement; its line is
/// `{"stage":"transcriptions","issuer","entries","accepted","single_reader","check":"pass"}`. The documents'
/// bytes are checked at run time, not here.
///
/// `registries/metrics.toml` must parse and pass its link rules (BD-060), one line:
/// `{"stage":"metrics","definitions","check":"pass"}`.
pub fn check_sources_registry(root: &Path) -> Result<Vec<String>, Vec<String>> {
    let dir = root.join("registries").join("sources");
    let entries =
        fs::read_dir(&dir).map_err(|e| vec![format!("cannot read {}: {e}", dir.display())])?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(vec![format!(
            "no *.toml source registries in {}",
            dir.display()
        )]);
    }
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for p in &files {
        let reg = match fmine_registry::sources::load_sources_registry(p) {
            Ok(reg) => reg,
            Err(e) => {
                errors.push(format!("registry-check {}: {e}", p.display()));
                continue;
            }
        };
        let issuer = reg.issuer.replace('\\', "\\\\").replace('"', "\\\"");
        lines.push(format!(
            "{{\"stage\":\"registry\",\"issuer\":\"{issuer}\",\"sources\":{},\"subjects\":{},\"reconstructions\":{},\
             \"label_checks\":{},\"mine_stats_labels\":{},\"documents\":{},\"check\":\"pass\"}}",
            reg.sources.len(),
            reg.subjects.len(),
            reg.reconstructions.len(),
            reg.reconstructions
                .iter()
                .map(|r| r.label_checks.len())
                .sum::<usize>(),
            reg.mine_stats_labels.len(),
            reg.documents.len()
        ));
        let tp = root
            .join("registries")
            .join("transcriptions")
            .join(format!("{}.toml", reg.issuer));
        if !tp.is_file() {
            continue;
        }
        match fmine_registry::transcriptions::load_transcriptions(&tp, &reg.issuer, &reg.documents)
        {
            Err(e) => errors.push(format!("registry-check {}: {e}", tp.display())),
            Ok(t) => {
                let a = fmine_registry::transcriptions::agree(&t);
                if a.is_consistent() {
                    lines.push(format!(
                        "{{\"stage\":\"transcriptions\",\"issuer\":\"{issuer}\",\"entries\":{},\"accepted\":{},\
                         \"single_reader\":{},\"check\":\"pass\"}}",
                        t.entries.len(),
                        a.accepted.len(),
                        a.single_reader.len()
                    ));
                } else {
                    let at: Vec<String> = a
                        .disagreements
                        .iter()
                        .map(|d| format!("{} ({})", d.locator, d.fields.join(", ")))
                        .chain(
                            a.quantity_clashes
                                .iter()
                                .map(|(doc, q, _)| format!("{doc} quantity {q} at two locators")),
                        )
                        .collect();
                    errors.push(format!(
                        "registry-check {}: readers disagree at {}",
                        tp.display(),
                        at.join("; ")
                    ));
                }
            }
        }
    }
    match fmine_registry::metrics::MetricRegistry::load_from_root(root) {
        Ok(reg) => lines.push(format!(
            "{{\"stage\":\"metrics\",\"definitions\":{},\"check\":\"pass\"}}",
            reg.definitions.len()
        )),
        Err(e) => errors.push(format!("registry-check metrics: {e}")),
    }
    if errors.is_empty() {
        Ok(lines)
    } else {
        Err(errors)
    }
}

/// Run all `GATE-LAYERS-00` checks (`BD-015`).
pub fn check_all(root: &Path) -> Result<LocReport, Vec<String>> {
    let mut all_errors = Vec::new();
    let loc_report = match check_loc_budget(root) {
        Ok(r) => Some(r),
        Err(e) => {
            all_errors.extend(e);
            None
        }
    };
    if let Err(e) = check_layers(root) {
        all_errors.extend(e);
    }
    if let Err(e) = check_deps(root) {
        all_errors.extend(e);
    }
    if let Err(e) = check_caps(root) {
        all_errors.extend(e);
    }
    if let Err(e) = lifts::check_lifts(root) {
        all_errors.extend(e);
    }
    if let Err(e) = check_scope(root) {
        all_errors.extend(e);
    }
    if let Err(e) = check_claims(root) {
        all_errors.extend(e);
    }
    if let Err(e) = check_det(root) {
        all_errors.extend(e);
    }
    if let Err(e) = check_sources_registry(root) {
        all_errors.extend(e);
    }
    if let Err(e) = refusals::check_refusals(root) {
        all_errors.extend(e);
    }
    if let Err(e) = int_as::check_int_as_001a(root) {
        all_errors.extend(e);
    }
    if all_errors.is_empty() {
        Ok(loc_report.unwrap_or(LocReport {
            rows: Vec::new(),
            total_planned: 0,
            total_ceiling: 0,
            total_actual: 0,
        }))
    } else {
        Err(all_errors)
    }
}

fn strip_kv_str(line: &str, key: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    if k.trim() != key {
        return None;
    }
    let val = v.trim().trim_matches('"');
    Some(val.to_string())
}

fn strip_kv_usize(line: &str, key: &str) -> Option<usize> {
    let (k, v) = line.split_once('=')?;
    if k.trim() != key {
        return None;
    }
    v.trim().parse::<usize>().ok()
}

fn count_rust_loc(dir: &Path) -> usize {
    collect_rs_files(dir)
        .into_iter()
        .map(|p| {
            fs::read_to_string(p)
                .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0)
        })
        .sum()
}

fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.exists() {
        return out;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(entries) = fs::read_dir(d) {
            let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn workspace_member_manifests(root: &Path) -> Result<Vec<PathBuf>, Vec<String>> {
    let root_toml = root.join("Cargo.toml");
    let content = fs::read_to_string(&root_toml)
        .map_err(|e| vec![format!("failed to read {}: {e}", root_toml.display())])?;
    let mut members = Vec::new();
    let mut in_members = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with("members = [") {
            in_members = true;
            if line.ends_with(']') {
                for item in line
                    .trim_start_matches("members = [")
                    .trim_end_matches(']')
                    .split(',')
                {
                    let m = item.trim().trim_matches('"');
                    if !m.is_empty() {
                        members.push(root.join(m).join("Cargo.toml"));
                    }
                }
                in_members = false;
            }
        } else if in_members {
            if line.starts_with(']') {
                in_members = false;
            } else {
                let m = line.trim_end_matches(',').trim().trim_matches('"');
                if !m.is_empty() && !m.starts_with('#') {
                    members.push(root.join(m).join("Cargo.toml"));
                }
            }
        }
    }
    Ok(members.into_iter().filter(|p| p.exists()).collect())
}

fn parse_cargo_manifest(
    manifest_path: &Path,
) -> Result<(String, Vec<String>, Vec<String>), Vec<String>> {
    let text = fs::read_to_string(manifest_path)
        .map_err(|e| vec![format!("failed to read {}: {e}", manifest_path.display())])?;
    let mut name = String::new();
    let mut deps = Vec::new();
    let mut dev_deps = Vec::new();
    let mut section = "";
    // Inside a dependency table: whether it is dev-only and, for the dotted form `[dependencies.<key>]`, where
    // its one entry sits, so a later `package = "..."` line can name the real package.
    let mut table: Option<(bool, Option<usize>)> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim();
            table = dependency_table(section).map(|(dev, key)| {
                let list = if dev { &mut dev_deps } else { &mut deps };
                let at = key.map(|k| {
                    list.push(k);
                    list.len() - 1
                });
                (dev, at)
            });
            continue;
        }
        if section == "package" {
            if let Some(n) = strip_kv_str(line, "name") {
                name = n;
            }
            continue;
        }
        let Some((dev, at)) = table else {
            continue;
        };
        let list = if dev { &mut dev_deps } else { &mut deps };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match at {
            Some(i) => {
                if unquote(key) == "package"
                    && let Some(slot) = list.get_mut(i)
                {
                    *slot = unquote(value);
                }
            }
            None => list.push(renamed_package(value).unwrap_or_else(|| unquote(key))),
        }
    }
    Ok((name, deps, dev_deps))
}

/// The crate names of `deny.toml`'s `[bans] deny` rows, one `{ name = "...", ... }` row per line, so a row with
/// `wrappers` still bans its crate as a direct dependency. `name` rows elsewhere (`skip`, `[[licenses.exceptions]]`)
/// ban nothing, and a deny row this reader cannot read is an error, never a crate that passes (`BD-005`).
fn banned_crates(deny_text: &str) -> Result<BTreeSet<String>, String> {
    let mut banned = BTreeSet::new();
    let mut section = "";
    let mut in_deny = false;
    for (index, raw_line) in deny_text.lines().enumerate() {
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(code, _)| code)
            .trim();
        if line.is_empty() {
            continue;
        }
        if in_deny {
            if line.starts_with(']') {
                in_deny = false;
                continue;
            }
            let name = line
                .strip_prefix('{')
                .and_then(|row| row.trim_start().strip_prefix("name"))
                .and_then(|row| row.trim_start().strip_prefix('='))
                .and_then(quoted_prefix)
                .ok_or_else(|| {
                    format!("line {}: unreadable `[bans] deny` row `{line}`", index + 1)
                })?;
            banned.insert(name);
        } else if line.starts_with('[') {
            section = line;
        } else if section == "[bans]"
            && line
                .strip_prefix("deny")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .is_some_and(|value| value.trim() == "[")
        {
            in_deny = true;
        }
    }
    if in_deny {
        return Err("`[bans] deny` is never closed".to_string());
    }
    Ok(banned)
}

/// The non-empty TOML string, of either quote style, that opens `text`.
fn quoted_prefix(text: &str) -> Option<String> {
    let text = text.trim_start();
    let quote = text.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &text[1..];
    let name = &body[..body.find(quote)?];
    (!name.is_empty()).then(|| name.to_string())
}

/// A TOML key or simple string value without its quotes, of either style.
fn unquote(text: &str) -> String {
    text.trim().trim_matches(['"', '\'']).to_string()
}

/// Whether a manifest section is a dependency table, as `(dev-only, dotted key)`. Covers `dependencies`,
/// `build-dependencies` (counted as normal) and `dev-dependencies`, each also under `target.<spec>.`, and each in
/// the dotted form `<table>.<key>` (`BD-005`: a banned crate must not pass by its declaration form).
fn dependency_table(section: &str) -> Option<(bool, Option<String>)> {
    const KINDS: [(&str, bool); 3] = [
        ("dependencies", false),
        ("build-dependencies", false),
        ("dev-dependencies", true),
    ];
    let names_kind = |rest: &str| {
        KINDS.iter().any(|(kind, _)| {
            rest.strip_prefix(kind)
                .is_some_and(|after| after.is_empty() || after.starts_with('.'))
        })
    };
    let rest = match section.strip_prefix("target.") {
        // The spec may be quoted and hold dots (`target.'cfg(unix)'.dependencies`), so the table is the first
        // `.<kind>` that ends the header or is followed by a dotted key.
        Some(spec) => spec
            .match_indices('.')
            .map(|(i, _)| &spec[i + 1..])
            .find(|rest| names_kind(rest))?,
        None => section,
    };
    KINDS.iter().find_map(|&(kind, dev)| {
        let after = rest.strip_prefix(kind)?;
        if after.is_empty() {
            return Some((dev, None));
        }
        let key = after.strip_prefix('.')?;
        Some((dev, Some(unquote(key))))
    })
}

/// The real package of an inline-table entry renamed with `package = "..."`, as in `rt = { package = "tokio" }`.
fn renamed_package(value: &str) -> Option<String> {
    let inner = value.trim().strip_prefix('{')?.trim_end_matches('}');
    inner.split(',').find_map(|kv| {
        let (key, value) = kv.split_once('=')?;
        (unquote(key) == "package").then(|| unquote(value))
    })
}

fn parse_allowed_modules(contract: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in contract.lines() {
        if line.contains("Allowed Module Families:")
            && let Some((_, rhs)) = line.split_once(':')
        {
            let cleaned = rhs.replace(['*', '`', '[', ']'], "");
            for token in cleaned.split(',') {
                let m = token.trim().trim_matches('"');
                if !m.is_empty() {
                    out.insert(m.to_string());
                }
            }
        }
    }
    out
}

fn strip_cfg_test(source: &str) -> String {
    if let Some(idx) = source.find("#[cfg(test)]") {
        source[..idx].to_string()
    } else {
        source.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seeded_upward_layer_edge_fails() {
        let allowed = BTreeSet::new();
        let res = validate_layer_edge("fmine-core", "S0", 0, "fmine-cli", "U", 9, &allowed);
        assert!(res.is_err());
    }

    #[test]
    fn test_seeded_open_child_region_fails() {
        let res = check_caps_in_source(
            "fmine-plan",
            "K3",
            Path::new("crates/fmine-plan/src/lib.rs"),
            "pub fn bad() { cx.open_child_region(); }",
        );
        assert!(res.is_err());
    }

    /// BD-325, B(ii): every layer, not only the pure ones `L-3` already covers.
    #[test]
    fn test_seeded_detached_worker_or_timer_fails_outside_the_runtime() {
        let at = Path::new("crates/fmine-cli/src/lib.rs");
        for code in [
            "std::thread::spawn(f)",
            "thread::spawn(f)",
            "std::thread::Builder::new()",
            "std::thread::sleep(d)",
            "std::time::Instant::now()",
            "std::time::SystemTime::now()",
        ] {
            let err = check_caps_in_source("fmine-cli", "U", at, code).expect_err(code);
            assert!(err.contains("criterion 9"), "{err}");
        }
        for owner in ["fmine-runtime", "fmine-lab"] {
            assert_eq!(
                check_caps_in_source(owner, "U", at, "std::thread::spawn(f)"),
                Ok(())
            );
        }
        assert_eq!(
            check_caps_in_source("fmine-cli", "U", at, "let t = now;"),
            Ok(())
        );
    }

    #[test]
    fn test_seeded_bare_f64_quantity_fails() {
        let res = check_bare_quantity_signature(
            Path::new("crates/fmine-plan/src/lib.rs"),
            "pub fn compute_cutoff(wet_tonnes: f64, grade_gpt: f64) -> f64 { wet_tonnes * grade_gpt }",
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_seeded_std_transcendental_in_bitwise_portable_fails() {
        let res = check_det_in_source(
            Path::new("crates/fmine-calib/src/lib.rs"),
            "K0",
            "//! Determinism: BitwisePortable\npub fn f(x: f64) -> f64 { f64::exp(x) }",
        );
        assert!(res.is_err());
    }

    const CALIB: &str = "crates/fmine-calib/src/lib.rs";
    const BITWISE: &str = "//! Determinism: BitwisePortable\n";

    fn det_check(path: &str, layer: &str, body: &str) -> Result<Option<String>, String> {
        let code = format!("{BITWISE}{body}");
        check_det_in_source(Path::new(path), layer, &code).map(|c| c.map(str::to_string))
    }

    // bd-hp2j.1: the old scan matched only `f64::{sin,cos,exp,ln,atan2}(`, so `x.ln()` passed.
    #[test]
    fn test_seeded_method_call_ln_in_bitwise_portable_fails() {
        let err = det_check(CALIB, "K0", "pub fn f(x: f64) -> f64 { x.ln() }").expect_err("x.ln()");
        assert!(err.contains("ln at line 2"), "{err}");
    }

    #[test]
    fn test_seeded_ufcs_f64_exp_in_bitwise_portable_fails() {
        let err = det_check(CALIB, "K0", "pub fn f(x: f64) -> f64 { f64::exp(x) }")
            .expect_err("f64::exp");
        assert!(err.contains("exp at line 2"), "{err}");
    }

    #[test]
    fn test_powi_banned_but_sqrt_and_mul_add_allowed() {
        assert!(det_check(CALIB, "K0", "pub fn f(x: f64) -> f64 { x.powi(4) }").is_err());
        let ok = "pub fn f(x: f64) -> f64 { x.sqrt().mul_add(x.abs(), x.floor().max(x.ceil().min(x.round()))) }";
        assert_eq!(
            det_check(CALIB, "K0", ok),
            Ok(Some("BitwisePortable".to_string()))
        );
    }

    #[test]
    fn test_exp2_not_matched_as_exp_but_banned_itself() {
        let calls = banned_transcendental_calls("let y = x.exp2(); let z = x.exp_m1();");
        assert_eq!(calls, vec![(1, "exp2"), (1, "exp_m1")]);
        assert!(det_check(CALIB, "K0", "pub fn f(x: f64) -> f64 { x.exp2() }").is_err());
    }

    #[test]
    fn test_comment_mentioning_ln_is_ignored() {
        let body = "/// Uses `x.ln()` in prose only.\npub fn f(x: f64) -> f64 { x * 2.0 } // not x.exp()\n";
        assert_eq!(
            det_check(CALIB, "K0", body),
            Ok(Some("BitwisePortable".to_string()))
        );
        // Two calls on one line are two matches.
        assert_eq!(banned_transcendental_calls("a.ln() - b.ln()").len(), 2);
    }

    #[test]
    fn test_missing_header_in_k_layer_fails() {
        let code = "pub fn f(x: f64) -> f64 { x }";
        for layer in ["K0", "K3", "E"] {
            let err = check_det_in_source(Path::new(CALIB), layer, code).expect_err(layer);
            assert!(
                err.contains("no `//! Determinism: <class>` header"),
                "{err}"
            );
        }
        // Outside K and E a header is optional.
        assert_eq!(
            check_det_in_source(Path::new("crates/fmine-cli/src/lib.rs"), "U", code),
            Ok(None)
        );
    }

    #[test]
    fn test_unknown_class_fails() {
        let code = "//! Determinism: Bitwise\npub fn f() {}";
        let err = check_det_in_source(Path::new(CALIB), "K0", code).expect_err("unknown class");
        assert!(err.contains("unknown determinism class `Bitwise`"), "{err}");
    }

    /// Only fmine-core's det.rs implements transcendentals; the old rule exempted any file named det.rs.
    #[test]
    fn test_only_core_det_rs_is_exempt() {
        let body = "pub fn ln(x: f64) -> f64 { x.ln() }";
        assert!(det_check(DET_EXEMPT, "S0", body).is_ok());
        assert!(det_check("crates/fmine-calib/src/det.rs", "K0", body).is_err());
    }

    /// The old rule read an unreadable file as empty, and so passed it.
    #[test]
    fn test_unreadable_file_is_error() {
        let dir = std::env::temp_dir().join(format!("xtask_det_unreadable_{}", std::process::id()));
        let src = dir.join("crates/fmine-calib/src");
        fs::create_dir_all(&src).expect("mkdir");
        fs::write(src.join("lib.rs"), "//! Determinism: DeterministicPerIsa\n")
            .expect("write lib.rs");
        // Not UTF-8, so it cannot be read as source.
        fs::write(src.join("broken.rs"), [0xff, 0xfe, 0x00]).expect("write broken.rs");
        let mut report = DetReport {
            files: 0,
            bitwise_files: 0,
        };
        let mut errors = Vec::new();
        check_det_dir(&dir, "K0", &src, &mut report, &mut errors);
        fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(report.files, 1);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].starts_with("crates/fmine-calib/src/broken.rs: unreadable"),
            "{errors:?}"
        );
    }

    /// The workspace passes, with every K and E file classified, and prints the bd-hp2j.1 log line.
    #[test]
    fn test_workspace_check_det_passes_and_logs() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let report = check_det(&root).expect("check-det passes on the workspace");
        println!("{}", report.log_line());
        // Every crates/<name>/src/**/*.rs except xtask's, counted by walking the directories.
        fn walk(dir: &Path) -> usize {
            fs::read_dir(dir).map_or(0, |entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .map(|p| {
                        if p.is_dir() {
                            walk(&p)
                        } else {
                            usize::from(p.extension().is_some_and(|x| x == "rs"))
                        }
                    })
                    .sum()
            })
        }
        let expected: usize = fs::read_dir(root.join("crates"))
            .expect("crates/")
            .flatten()
            .filter(|e| e.file_name() != "xtask")
            .map(|e| walk(&e.path().join("src")))
            .sum();
        assert_eq!(report.files, expected, "{report:?}");
        assert_eq!(report.bitwise_files, 7, "{report:?}");
    }

    /// A broken registry fails `check-all`, naming the file and line; the workspace's own registry passes.
    #[test]
    fn test_registry_check_on_workspace_and_on_a_broken_copy() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let lines =
            check_sources_registry(&root).expect("workspace registries pass registry-check");
        let text = fs::read_to_string(root.join("registries/sources/barrick.toml")).expect("read");
        // The bd-s9j3.1 log line, its counts taken from the TOML text rather than from the parser. A label-check
        // set counts once per reconstruction that names it.
        let count = |table: &str| text.lines().filter(|l| l.trim() == table).count();
        let applied: usize = text
            .lines()
            .filter_map(|l| l.strip_prefix("label_checks = "))
            .map(|set| {
                let row = format!("set = {set}");
                text.lines().filter(|l| l.trim() == row).count()
            })
            .sum();
        assert_eq!(
            lines[0],
            format!(
                "{{\"stage\":\"registry\",\"issuer\":\"barrick\",\"sources\":{},\"subjects\":{},\
                 \"reconstructions\":{},\"label_checks\":{},\"mine_stats_labels\":{},\"documents\":{},\
                 \"check\":\"pass\"}}",
                count("[[source]]"),
                count("[[subject]]"),
                count("[[reconstruction]]"),
                applied,
                count("[[mine_stats_label]]"),
                count("[[document]]")
            )
        );
        // A local (git-ignored) transcription registry adds its own line; CI and a fresh clone have none.
        let local = root
            .join("registries/transcriptions/barrick.toml")
            .is_file();
        let metrics = fs::read_to_string(root.join("registries/metrics.toml")).expect("metrics");
        let definitions = metrics
            .lines()
            .filter(|l| l.trim() == "[[definition]]")
            .count();
        assert_eq!(
            lines.last().expect("metrics registry log"),
            &format!("{{\"stage\":\"metrics\",\"definitions\":{definitions},\"check\":\"pass\"}}")
        );
        assert_eq!(lines.len(), 2 + usize::from(local), "{lines:?}");
        let scratch = std::env::temp_dir().join(format!("xtask-registry-{}", std::process::id()));
        let dir = scratch.join("registries").join("sources");
        fs::create_dir_all(&dir).expect("scratch dir");
        fs::write(scratch.join("registries/metrics.toml"), &metrics).expect("metrics fixture");
        let dup = text.replacen(
            "sha256 = \"1a73d6d7d4658f923757e873eec147ccc74aa8aada6a439f3c98da8540ff68d5\"",
            "sha256 = \"e2e1243ec0549f4c487c773869876e2acade401a0b6cd1141adf4b9296aec840\"",
            1,
        );
        assert_ne!(dup, text, "the edit must apply");
        fs::write(dir.join("barrick.toml"), dup).expect("write");
        let errs = check_sources_registry(&scratch).expect_err("duplicate digest");
        assert!(
            errs[0].contains("barrick.toml: line ") && errs[0].contains("duplicate sha256"),
            "{errs:?}"
        );
        fs::write(dir.join("other.toml"), &text).expect("write");
        let errs = check_sources_registry(&scratch).expect_err("issuer/file-name mismatch");
        assert!(
            errs.iter()
                .any(|e| e.contains("does not match the file name")),
            "{errs:?}"
        );
        let _ = fs::remove_dir_all(&scratch);

        // bd-plx1: a transcription registry beside the sources registry is checked against its documents and
        // the two-reader rule. Synthetic rows (labels and tokens are invented) on a registered document.
        let scratch = std::env::temp_dir().join(format!("xtask-transcribe-{}", std::process::id()));
        fs::create_dir_all(scratch.join("registries/sources")).expect("scratch dir");
        fs::create_dir_all(scratch.join("registries/transcriptions")).expect("scratch dir");
        fs::write(scratch.join("registries/sources/barrick.toml"), &text).expect("write");
        fs::write(scratch.join("registries/metrics.toml"), &metrics).expect("metrics fixture");
        let sha = "63b3af0b98436900159f6ddac7f2920a0178f0e8651f1ee262d07bbaa690d281";
        let entry = |reader: &str, token: &str| {
            format!(
                "\n[[entry]]\ndoc = \"Barrick_Q4_2025_MD-A.pdf\"\ndoc_sha256 = \"{sha}\"\npage = 1\n\
                 table = \"Synthetic table\"\nrow = \"Synthetic row\"\ncol = \"Synthetic column\"\n\
                 token = \"{token}\"\nquantity = \"synthetic\"\nunit = \"none\"\nbasis = \"none\"\n\
                 period = \"none\"\nreader = \"{reader}\"\n"
            )
        };
        let write = |rows: &[String]| {
            fs::write(
                scratch.join("registries/transcriptions/barrick.toml"),
                format!(
                    "schema = \"fmine.transcriptions.v1\"\nissuer = \"barrick\"\n{}",
                    rows.concat()
                ),
            )
            .expect("write");
        };
        write(&[entry("reader_a", "1"), entry("reader_b", "1")]);
        let lines = check_sources_registry(&scratch).expect("agreeing rows pass");
        assert_eq!(
            lines[1],
            "{\"stage\":\"transcriptions\",\"issuer\":\"barrick\",\"entries\":2,\"accepted\":1,\
             \"single_reader\":0,\"check\":\"pass\"}"
        );
        write(&[entry("reader_a", "1"), entry("reader_b", "2")]);
        let errs = check_sources_registry(&scratch).expect_err("disagreeing rows fail");
        assert!(
            errs[0].contains("readers disagree at Barrick_Q4_2025_MD-A.pdf:p1:")
                && errs[0].ends_with("(token)"),
            "{errs:?}"
        );
        write(&[entry("reader_a", "1").replace(sha, &"0".repeat(64))]);
        let errs = check_sources_registry(&scratch).expect_err("other bytes fail");
        assert!(errs[0].contains("read from other bytes"), "{errs:?}");
        let _ = fs::remove_dir_all(&scratch);
    }

    /// bd-s9j3.2: the candidate row `scripts/register_vintage.py` prints for a new vintage parses in the TOML
    /// subset and passes registry-check when appended to a copy of the workspace registry, with no other edit.
    #[test]
    fn test_candidate_row_parses_under_toml_subset() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let prefix = "https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/";
        let sha = "ab".repeat(32);
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(root.join("scripts/register_vintage.py"))
            .args(["row", "barrick", prefix])
            .arg(format!("{prefix}2026/q3/AISC_Q3_2026.xlsx"))
            .args(["51234", &sha, "2026-11-10T11:05:00Z"])
            .output()
            .expect("python3 runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let row = String::from_utf8(out.stdout).expect("utf-8");
        for want in [
            "[[source]]\n",
            "file = \"Barrick_Q3_2026_AISC.xlsx\"\n",
            "kind = \"aisc_reconciliation\"\n",
            "quarter = \"2026Q3\"\n",
            "path = \"2026/q3/AISC_Q3_2026.xlsx\"\n",
            "url_basis = \"fetched and verified 2026-11-10\"\n",
            "bytes = 51_234\n",
        ] {
            assert!(row.contains(want), "{want:?} not in {row}");
        }
        fmine_registry::toml_subset::parse_subset(&row).expect("the row is in the TOML subset");

        let text = fs::read_to_string(root.join("registries/sources/barrick.toml")).expect("read");
        let scratch =
            std::env::temp_dir().join(format!("xtask-vintage-row-{}", std::process::id()));
        let dir = scratch.join("registries").join("sources");
        fs::create_dir_all(&dir).expect("scratch dir");
        fs::write(dir.join("barrick.toml"), format!("{text}\n{row}")).expect("write");
        fs::copy(
            root.join("registries/metrics.toml"),
            scratch.join("registries/metrics.toml"),
        )
        .expect("metrics fixture");
        let lines =
            check_sources_registry(&scratch).expect("registry-check passes with the row appended");
        let sources = text.lines().filter(|l| l.trim() == "[[source]]").count();
        assert!(
            lines[0].contains(&format!("\"sources\":{},", sources + 1)),
            "{lines:?}"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    /// BD-008 done-when "smuggled module fails CI": a module missing from the charter's Allowed Module Families
    /// fails `check_scope` whatever its visibility; the `#[cfg(test)]` module and a module nested in it pass.
    #[test]
    fn test_seeded_smuggled_module_fails_check_scope() {
        let scratch = std::env::temp_dir().join(format!("xtask-scope-{}", std::process::id()));
        let krate = scratch.join("crates").join("x");
        fs::create_dir_all(krate.join("src")).expect("scratch crate");
        fs::write(
            scratch.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/x\"]\n",
        )
        .expect("workspace manifest");
        fs::write(krate.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("manifest");
        let mut contract = String::from("- **Allowed Module Families:** `[\"core\"]`\n");
        for sec in 1..=10 {
            contract.push_str(&format!("## {sec}. Section\n"));
        }
        fs::write(krate.join("CONTRACT.md"), contract).expect("contract");
        let clean = "pub mod core;\n\n#[cfg(test)]\n#[allow(dead_code)]\nmod tests {\n    mod helpers {}\n}\n";
        fs::write(krate.join("src/lib.rs"), clean).expect("lib.rs");
        check_scope(&scratch).expect("charted modules and the test module pass");
        for smuggled in [
            "pub mod smuggled;",
            "mod smuggled;",
            "pub(crate) mod smuggled {}",
            "pub(super) mod smuggled;",
        ] {
            fs::write(krate.join("src/lib.rs"), format!("{clean}{smuggled}\n")).expect("lib.rs");
            let errs = check_scope(&scratch).expect_err(smuggled);
            assert!(
                errs.len() == 1 && errs[0].contains("undeclared module `smuggled` in x/src/lib.rs"),
                "{smuggled}: {errs:?}"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }

    /// BD-005: a member declaring tokio fails `check_deps` in every manifest form, with the workspace's own
    /// deny.toml, and a name that only starts like it passes. A `wrappers` row still bans its crate, a `skip` row
    /// bans nothing, and a deny.toml with no readable rows, or one unreadable deny row, fails instead of passing
    /// every crate. Tokio reached transitively is `int_as`'s B(i) check, which `check-all` also runs.
    #[test]
    fn test_seeded_tokio_fails_check_deps_in_every_declaration_form() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        assert_eq!(check_deps(&root), Ok(()));
        let forms = [
            ("plain", "[dependencies]\ntokio = \"1\"\n"),
            ("quoted", "[dependencies]\n\"tokio\" = '1'\n"),
            (
                "dotted",
                "[dependencies.tokio]\nversion = \"1\"\nfeatures = [\"rt\"]\n",
            ),
            (
                "target",
                "[target.'cfg(unix)'.dependencies]\ntokio = \"1\"\n",
            ),
            (
                "renamed",
                "[dependencies]\nrt = { version = \"1\", package = \"tokio\" }\n",
            ),
            (
                "dotted-renamed",
                "[dependencies.rt]\npackage = 'tokio'\nversion = \"1\"\n",
            ),
            ("build", "[build-dependencies]\ntokio = \"1\"\n"),
            (
                "dev",
                "[target.\"x86_64-unknown-linux-gnu\".dev-dependencies]\ntokio = \"1\"\n",
            ),
            (
                "clean",
                "[dependencies]\ntokio-lookalike = \"1\"\n\n[dependencies.rt]\nversion = \"1\"\n",
            ),
        ];
        let scratch = std::env::temp_dir().join(format!("xtask-deps-{}", std::process::id()));
        fs::create_dir_all(&scratch).expect("scratch dir");
        let members: Vec<String> = forms
            .iter()
            .map(|(name, _)| format!("\"crates/{name}\""))
            .collect();
        fs::write(
            scratch.join("Cargo.toml"),
            format!("[workspace]\nmembers = [{}]\n", members.join(", ")),
        )
        .expect("workspace manifest");
        fs::copy(root.join("deny.toml"), scratch.join("deny.toml")).expect("deny.toml");
        for (name, deps) in forms {
            let krate = scratch.join("crates").join(name);
            fs::create_dir_all(&krate).expect("scratch crate");
            fs::write(
                krate.join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\n\n{deps}"),
            )
            .expect("manifest");
        }
        let want: Vec<String> = forms[..forms.len() - 1]
            .iter()
            .map(|(name, _)| {
                format!("banned dependency tokio in crate {name} (violates L-5 / deny.toml)")
            })
            .collect();
        assert_eq!(check_deps(&scratch), Err(want));

        // The workspace's `wrappers` rows still ban time and chrono as direct dependencies, and a `skip` row bans
        // nothing (the R7-C150 skips are crates a member may name).
        fs::write(
            scratch.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/wrapped\"]\n",
        )
        .expect("workspace manifest");
        let krate = scratch.join("crates").join("wrapped");
        fs::create_dir_all(&krate).expect("scratch crate");
        fs::write(
            krate.join("Cargo.toml"),
            "[package]\nname = \"wrapped\"\n\n[dependencies]\nchrono = \"0.4\"\nsha2 = \"0.10\"\ntime = \"0.3\"\n",
        )
        .expect("manifest");
        let wrapped = |dep: &str| {
            format!("banned dependency {dep} in crate wrapped (violates L-5 / deny.toml)")
        };
        assert_eq!(
            check_deps(&scratch),
            Err(vec![wrapped("chrono"), wrapped("time")])
        );
        fs::write(
            scratch.join("deny.toml"),
            "[bans]\nskip = [\n    { name = \"sha2\" },\n]\ndeny = [\n    { name = \"time\", wrappers = [\"fp-types\"] }, # R7-C152\n]\n",
        )
        .expect("deny.toml");
        assert_eq!(check_deps(&scratch), Err(vec![wrapped("time")]));
        fs::write(
            scratch.join("deny.toml"),
            "[bans]\ndeny = [\n    { name = \"time\" },\n    \"chrono\",\n]\n",
        )
        .expect("deny.toml");
        let errs = check_deps(&scratch).expect_err("an unreadable deny row");
        assert!(
            errs.len() == 1
                && errs[0].contains("line 4: unreadable `[bans] deny` row `\"chrono\",`"),
            "{errs:?}"
        );
        fs::write(
            scratch.join("deny.toml"),
            "[bans]\ndeny = [\n    { name = \"time\" },\n",
        )
        .expect("deny.toml");
        let errs = check_deps(&scratch).expect_err("an unclosed deny list");
        assert!(
            errs.len() == 1 && errs[0].contains("`[bans] deny` is never closed"),
            "{errs:?}"
        );

        fs::write(scratch.join("deny.toml"), "[bans]\ndeny = [\"tokio\"]\n").expect("deny.toml");
        let errs = check_deps(&scratch).expect_err("an unread deny list");
        assert!(
            errs.len() == 1 && errs[0].contains("no banned crate read"),
            "{errs:?}"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn test_workspace_check_all_gate_layers_00_passes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let report = check_all(&root).expect("GATE-LAYERS-00 check-all must pass with zero errors");
        assert!(report.rows.len() >= 59);
        assert_eq!(report.total_planned, 1_128_000);
        assert_eq!(report.total_ceiling, 1_298_000);
        assert!(report.total_actual > 4_000);
    }
}
