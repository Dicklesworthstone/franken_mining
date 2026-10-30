//! FrankenMining `xtask` governance and architectural verification library (`GATE-LAYERS-00`, BD-003..BD-015).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

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
            if let Some((k, v)) = line.split_once('=') {
                if let Ok(r) = v.trim().parse::<usize>() {
                    ranks.insert(k.trim().to_string(), r);
                }
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
    if from_rank == to_rank && from_layer != "T" {
        if !allowed_same_layer.contains(&(from_crate.to_string(), to_crate.to_string())) {
            return Err(format!(
                "undeclared same-layer dependency (L-1): {from_crate} ({from_layer}) -> {to_crate} ({to_layer}); register in workspace_topology.toml"
            ));
        }
    }
    Ok(())
}

/// Validate that no workspace crate pulls a banned dependency from `deny.toml` (`L-5`, `BD-005`)
/// and no production crate depends on a `T`-layer oracle crate (`GATE-ORACLE-01`, `BD-013`).
pub fn check_deps(root: &Path) -> Result<(), Vec<String>> {
    let deny_path = root.join("deny.toml");
    let deny_text = fs::read_to_string(&deny_path)
        .map_err(|e| vec![format!("failed to read {}: {e}", deny_path.display())])?;

    let mut banned = BTreeSet::new();
    for raw_line in deny_text.lines() {
        let line = raw_line.trim();
        if line.starts_with("{ name =") {
            if let Some(val) = strip_kv_str(
                line.trim_matches(|c| c == '{' || c == '}' || c == ','),
                "name",
            ) {
                banned.insert(val);
            }
        }
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
    Ok(())
}

/// Check `registries/lifts.toml` against provenance headers in lifted files (`L-7`, `BD-007`).
pub fn check_lifts(root: &Path) -> Result<(), Vec<String>> {
    let lifts_path = root.join("registries/lifts.toml");
    let text = fs::read_to_string(&lifts_path)
        .map_err(|e| vec![format!("failed to read {}: {e}", lifts_path.display())])?;

    let mut errors = Vec::new();
    let mut source = String::new();
    let mut commit = String::new();
    let mut target = String::new();
    let mut in_lift = false;

    let verify_one = |src: &str, rev: &str, tgt: &str, errs: &mut Vec<String>| {
        if tgt.is_empty() {
            return;
        }
        let target_path = root.join(tgt);
        if !target_path.exists() {
            // Target crate may not be sliced yet; only enforce once target file exists.
            return;
        }
        let content = fs::read_to_string(&target_path).unwrap_or_default();
        let first_line = content.lines().next().unwrap_or("");
        let expected_prefix = format!("//! Provenance: frankensim {src} @ {rev};");
        if !first_line.starts_with(&expected_prefix) {
            errs.push(format!(
                "lift header mismatch in {}: expected first line starting with `{expected_prefix}`, found `{first_line}`",
                target_path.display()
            ));
        }
    };

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line == "[[lift]]" {
            if in_lift {
                verify_one(&source, &commit, &target, &mut errors);
            }
            source.clear();
            commit.clear();
            target.clear();
            in_lift = true;
        } else if in_lift {
            if let Some(v) = strip_kv_str(line, "source") {
                source = v;
            } else if let Some(v) = strip_kv_str(line, "commit") {
                commit = v;
            } else if let Some(v) = strip_kv_str(line, "target") {
                target = v;
            }
        }
    }
    if in_lift {
        verify_one(&source, &commit, &target, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check that every workspace crate has a 10-section `CONTRACT.md` and every `pub mod` in `src/lib.rs`
/// is listed in `Allowed Module Families` (`BD-002`, `BD-008`).
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
            for line in lib_text.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("pub mod ") {
                    let mod_name = rest
                        .trim_end_matches(';')
                        .split_whitespace()
                        .next()
                        .unwrap_or("");
                    if !mod_name.is_empty() && !allowed_mods.contains(mod_name) {
                        errors.push(format!(
                            "undeclared module `pub mod {mod_name}` in {pkg_name}/src/lib.rs not in CONTRACT.md Allowed Module Families {:?} (BD-008)",
                            allowed_mods
                        ));
                    }
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

/// Check `PO-NUM-2` (`BD-010`): ban direct `f64::sin`, `f64::cos`, `f64::exp`, `f64::ln`, `f64::atan2`
/// in modules marked `//! Determinism: BitwisePortable`.
pub fn check_det(root: &Path) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for member_toml in workspace_member_manifests(root)? {
        let (pkg_name, _, _) = parse_cargo_manifest(&member_toml)?;
        if pkg_name == "xtask" {
            continue;
        }
        let src_dir = member_toml.parent().unwrap_or(root).join("src");
        for rs_file in collect_rs_files(&src_dir) {
            if rs_file.ends_with("det.rs") {
                continue;
            }
            let content = fs::read_to_string(&rs_file).unwrap_or_default();
            let prod_code = strip_cfg_test(&content);
            if let Err(e) = check_det_in_source(&rs_file, &prod_code) {
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

/// Inspect a `BitwisePortable` module for banned `std` floating-point transcendentals (`PO-NUM-2`, `BD-010`).
pub fn check_det_in_source(path: &Path, prod_code: &str) -> Result<(), String> {
    if !prod_code.contains("Determinism: BitwisePortable") {
        return Ok(());
    }
    for banned in [
        "f64::sin(",
        "f64::cos(",
        "f64::exp(",
        "f64::ln(",
        "f64::atan2(",
    ] {
        if prod_code.contains(banned) {
            return Err(format!(
                "{}: banned std transcendental `{banned}` in BitwisePortable module (PO-NUM-2; use fmine_core::det)",
                path.display()
            ));
        }
    }
    Ok(())
}

/// `registry-check` (bd-s9j3.1): every `registries/sources/*.toml` parses in the TOML subset and passes the
/// source-registry rules (unique files, paths and digests; lowercase 64-hex digests; `YYYYQn` quarters;
/// registered kinds, URL bases and families; reconstructions pointing at registered rows), and its `issuer`
/// matches its file name. A missing directory is an error: the public lane cannot run without it.
pub fn check_sources_registry(root: &Path) -> Result<(), Vec<String>> {
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
    let errors: Vec<String> = files
        .iter()
        .filter_map(|p| {
            fmine_registry::sources::load_sources_registry(p)
                .err()
                .map(|e| format!("registry-check {}: {e}", p.display()))
        })
        .collect();
    if errors.is_empty() {
        Ok(())
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
    if let Err(e) = check_lifts(root) {
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
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        if section == "package" {
            if let Some(n) = strip_kv_str(line, "name") {
                name = n;
            }
        } else if section == "dependencies" {
            if let Some((k, _)) = line.split_once('=') {
                deps.push(k.trim().to_string());
            }
        } else if section == "dev-dependencies" {
            if let Some((k, _)) = line.split_once('=') {
                dev_deps.push(k.trim().to_string());
            }
        }
    }
    Ok((name, deps, dev_deps))
}

fn parse_allowed_modules(contract: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in contract.lines() {
        if line.contains("Allowed Module Families:") {
            if let Some((_, rhs)) = line.split_once(':') {
                let cleaned = rhs
                    .replace('*', "")
                    .replace('`', "")
                    .replace('[', "")
                    .replace(']', "");
                for token in cleaned.split(',') {
                    let m = token.trim().trim_matches('"');
                    if !m.is_empty() {
                        out.insert(m.to_string());
                    }
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
            "//! Determinism: BitwisePortable\npub fn f(x: f64) -> f64 { f64::exp(x) }",
        );
        assert!(res.is_err());
    }

    /// A broken registry fails `check-all`, naming the file and line; the workspace's own registry passes.
    #[test]
    fn test_registry_check_on_workspace_and_on_a_broken_copy() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        check_sources_registry(&root).expect("workspace registries pass registry-check");
        let scratch = std::env::temp_dir().join(format!("xtask-registry-{}", std::process::id()));
        let dir = scratch.join("registries").join("sources");
        fs::create_dir_all(&dir).expect("scratch dir");
        let text = fs::read_to_string(root.join("registries/sources/barrick.toml")).expect("read");
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
        fs::write(dir.join("other.toml"), text).expect("write");
        let errs = check_sources_registry(&scratch).expect_err("issuer/file-name mismatch");
        assert!(
            errs.iter()
                .any(|e| e.contains("does not match the file name")),
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
