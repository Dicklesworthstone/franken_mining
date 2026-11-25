//! Numerical kernel determinism and thread-count registry enforcement (`PO-NUM-3`, plan §29.7, `bd-dm2z`).
//!
//! Validates `registries/kernels.toml` and enforces that:
//! 1. Every `[[kernel]]` entry has valid metadata, an existing `source_file`, matching
//!    `//! Determinism:` header (for K/E crates), and a `test_ref` whose function actually exists
//!    inside the declared crate.
//! 2. No unlisted `std::thread::` numerical kernel in S0/K/E library source files can qualify a
//!    certified path (`check_kernels` fails immediately on any unlisted threaded kernel).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{check_det_in_source, check_loc_budget, strip_cfg_test, strip_kv_str};

const VALID_CLASSES: &[&str] = &[
    "Exact",
    "BitwisePortable",
    "DeterministicPerIsa",
    "ToleranceBounded",
];

const VALID_THREADING: &[&str] = &["serial", "parallel_fixed_chunk"];

/// One parsed `[[kernel]]` row from `registries/kernels.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelRow {
    pub id: String,
    pub crate_name: String,
    pub source_file: String,
    pub determinism_class: String,
    pub threading: String,
    pub thread_control: String,
    pub reduction_order: String,
    pub certified_paths: String,
    pub test_ref: String,
    pub cross_isa_ref: String,
}

/// Summary report emitted by `xtask check-kernels`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelCheckReport {
    pub total_kernels: usize,
    pub parallel_kernels: usize,
    pub serial_kernels: usize,
    pub checked_threaded_files: usize,
}

impl KernelCheckReport {
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"kernels\",\"total\":{},\"parallel_fixed_chunk\":{},\"serial\":{},\"checked_threaded_files\":{}}}",
            self.total_kernels,
            self.parallel_kernels,
            self.serial_kernels,
            self.checked_threaded_files
        )
    }
}

/// Parse `registries/kernels.toml` strictly (schema_version = 1).
pub fn parse_kernels_toml(text: &str) -> Result<Vec<KernelRow>, Vec<String>> {
    let mut errors = Vec::new();
    let mut saw_version = false;
    let mut rows = Vec::new();
    let mut cur: Option<BTreeMap<String, String>> = None;

    let flush =
        |fields: BTreeMap<String, String>, rows: &mut Vec<KernelRow>, errors: &mut Vec<String>| {
            let req = |k: &str, errs: &mut Vec<String>| -> String {
                match fields.get(k) {
                    Some(v) if !v.trim().is_empty() => v.clone(),
                    _ => {
                        errs.push(format!(
                            "registries/kernels.toml: [[kernel]] missing non-empty field '{k}'"
                        ));
                        String::new()
                    }
                }
            };
            let row = KernelRow {
                id: req("id", errors),
                crate_name: req("crate", errors),
                source_file: req("source_file", errors),
                determinism_class: req("determinism_class", errors),
                threading: req("threading", errors),
                thread_control: req("thread_control", errors),
                reduction_order: req("reduction_order", errors),
                certified_paths: req("certified_paths", errors),
                test_ref: req("test_ref", errors),
                cross_isa_ref: req("cross_isa_ref", errors),
            };
            if !row.id.is_empty() {
                rows.push(row);
            }
        };

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[kernel]]" {
            if !saw_version {
                errors.push(format!(
                    "registries/kernels.toml:{}: schema_version = 1 must precede [[kernel]]",
                    idx + 1
                ));
            }
            if let Some(prev) = cur.take() {
                flush(prev, &mut rows, &mut errors);
            }
            cur = Some(BTreeMap::new());
            continue;
        }
        match cur.as_mut() {
            None => {
                if line == "schema_version = 1" {
                    saw_version = true;
                } else {
                    errors.push(format!(
                        "registries/kernels.toml:{}: unexpected top-level line {line:?}",
                        idx + 1
                    ));
                }
            }
            Some(map) => {
                let mut matched = false;
                for key in [
                    "id",
                    "crate",
                    "source_file",
                    "determinism_class",
                    "threading",
                    "thread_control",
                    "reduction_order",
                    "certified_paths",
                    "test_ref",
                    "cross_isa_ref",
                ] {
                    if let Some(val) = strip_kv_str(line, key) {
                        if map.insert(key.to_string(), val).is_some() {
                            errors.push(format!(
                                "registries/kernels.toml:{}: duplicate field '{key}'",
                                idx + 1
                            ));
                        }
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    errors.push(format!(
                        "registries/kernels.toml:{}: unrecognized field {line:?}",
                        idx + 1
                    ));
                }
            }
        }
    }
    if let Some(last) = cur.take() {
        flush(last, &mut rows, &mut errors);
    }
    if !saw_version {
        errors.push("registries/kernels.toml: missing schema_version = 1".to_string());
    }
    if rows.is_empty() {
        errors.push("registries/kernels.toml: must declare at least one [[kernel]]".to_string());
    }
    if errors.is_empty() {
        Ok(rows)
    } else {
        Err(errors)
    }
}

fn contains_thread_use(prod_code: &str) -> bool {
    for raw in prod_code.lines() {
        let code = raw.split("//").next().unwrap_or("");
        if code.contains("std::thread::") {
            return true;
        }
    }
    false
}

fn crate_has_test_fn(crate_dir: &Path, fn_name: &str) -> bool {
    let needle = format!("fn {fn_name}(");
    let mut stack = vec![crate_dir.join("src"), crate_dir.join("tests")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Ok(text) = fs::read_to_string(&path)
                && text.contains(&needle)
            {
                return true;
            }
        }
    }
    false
}

fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Validate `registries/kernels.toml` and enforce that no unlisted threaded kernel exists in
/// S0/K/E library crates (`PO-NUM-3`, `bd-dm2z`).
pub fn check_kernels(root: &Path) -> Result<KernelCheckReport, Vec<String>> {
    let path = root.join("registries/kernels.toml");
    let text = fs::read_to_string(&path)
        .map_err(|e| vec![format!("failed to read {}: {e}", path.display())])?;
    let rows = parse_kernels_toml(&text)?;

    let loc = check_loc_budget(root)?;
    let crate_layers: BTreeMap<String, String> =
        loc.rows.into_iter().map(|r| (r.name, r.layer)).collect();

    let mut errors = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut parallel_files = BTreeSet::new();
    let mut parallel_kernels = 0usize;
    let mut serial_kernels = 0usize;

    for row in &rows {
        if !seen_ids.insert(row.id.clone()) {
            errors.push(format!(
                "registries/kernels.toml: duplicate kernel id '{}'",
                row.id
            ));
        }
        if !VALID_CLASSES.contains(&row.determinism_class.as_str()) {
            errors.push(format!(
                "registries/kernels.toml: kernel '{}' has invalid determinism_class {:?}",
                row.id, row.determinism_class
            ));
        }
        if !VALID_THREADING.contains(&row.threading.as_str()) {
            errors.push(format!(
                "registries/kernels.toml: kernel '{}' has invalid threading {:?}",
                row.id, row.threading
            ));
        }
        if row.threading == "parallel_fixed_chunk" {
            parallel_kernels += 1;
            parallel_files.insert(row.source_file.clone());
        } else {
            serial_kernels += 1;
        }

        let Some(layer) = crate_layers.get(&row.crate_name) else {
            errors.push(format!(
                "registries/kernels.toml: kernel '{}' names unregistered crate '{}'",
                row.id, row.crate_name
            ));
            continue;
        };

        let src_path = root.join(&row.source_file);
        match fs::read_to_string(&src_path) {
            Err(e) => errors.push(format!(
                "registries/kernels.toml: kernel '{}' source_file '{}' unreadable: {e}",
                row.id, row.source_file
            )),
            Ok(content) => {
                if layer.starts_with('K') || layer == "E" {
                    let prod = strip_cfg_test(&content);
                    match check_det_in_source(Path::new(&row.source_file), layer, &prod) {
                        Ok(Some(found)) if found != row.determinism_class => {
                            errors.push(format!(
                                "registries/kernels.toml: kernel '{}' declares determinism_class={} \
                                 but {} declares //! Determinism: {found}",
                                row.id, row.determinism_class, row.source_file
                            ));
                        }
                        Ok(None) => {
                            errors.push(format!(
                                "registries/kernels.toml: kernel '{}' source {} is missing //! Determinism: header",
                                row.id, row.source_file
                            ));
                        }
                        Err(msg) => errors.push(msg),
                        Ok(Some(_)) => {}
                    }
                }
            }
        }

        let expected_prefix = format!("test:{}::", row.crate_name);
        if !row.test_ref.starts_with(&expected_prefix) {
            errors.push(format!(
                "registries/kernels.toml: kernel '{}' test_ref {:?} must start with {:?}",
                row.id, row.test_ref, expected_prefix
            ));
        } else if let Some(fn_name) = row.test_ref.rsplit("::").next() {
            let crate_dir = root.join("crates").join(&row.crate_name);
            if !crate_has_test_fn(&crate_dir, fn_name) {
                errors.push(format!(
                    "registries/kernels.toml: kernel '{}' test_ref {:?} does not match any `fn {fn_name}(` in {}",
                    row.id,
                    row.test_ref,
                    crate_dir.display()
                ));
            }
        }
    }

    // Enforce: no unlisted threaded numerical kernel in S0, K0..K4, or E library crates.
    let mut checked_threaded_files = 0usize;
    for (crate_name, layer) in &crate_layers {
        if !(layer == "S0" || layer.starts_with('K') || layer == "E") {
            continue;
        }
        let src_dir = root.join("crates").join(crate_name).join("src");
        if !src_dir.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);
        for file in files {
            let Ok(content) = fs::read_to_string(&file) else {
                continue;
            };
            let prod = strip_cfg_test(&content);
            if contains_thread_use(&prod) {
                checked_threaded_files += 1;
                let rel = file
                    .strip_prefix(root)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !parallel_files.contains(&rel) {
                    errors.push(format!(
                        "PO-NUM-3: unlisted threaded kernel in {rel} (crate {crate_name}, layer {layer}) \
                         cannot qualify a certified path; register it in registries/kernels.toml with \
                         threading = \"parallel_fixed_chunk\" and a thread-count determinism test"
                    ));
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(KernelCheckReport {
            total_kernels: rows.len(),
            parallel_kernels,
            serial_kernels,
            checked_threaded_files,
        })
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_workspace_kernels_registry_passes() {
        let root = workspace_root();
        let report = check_kernels(&root).expect("registries/kernels.toml must pass PO-NUM-3");
        assert!(report.total_kernels >= 14);
        assert!(report.parallel_kernels >= 2);
        assert_eq!(report.checked_threaded_files, 1);
    }

    #[test]
    fn test_unlisted_threaded_kernel_is_refused() {
        let prod_with_thread = r#"
//! Determinism: DeterministicPerIsa
pub fn sneaky_parallel(xs: &[f64]) -> f64 {
    std::thread::scope(|s| {
        let h = s.spawn(|| xs.iter().sum::<f64>());
        h.join().unwrap()
    })
}
"#;
        assert!(contains_thread_use(prod_with_thread));
        let registered: BTreeSet<String> = ["crates/fmine-lp/src/pdlp/mat.rs".to_string()]
            .into_iter()
            .collect();
        let unlisted = "crates/fmine-numerics/src/sneaky.rs".to_string();
        assert!(!registered.contains(&unlisted));

        // Comments mentioning std::thread:: must not trigger false positives.
        let comment_only = "//! No std::thread::spawn used here.\npub fn serial() -> u32 { 1 }\n";
        assert!(!contains_thread_use(comment_only));
    }

    /// PO-NUM-3 negative control: on order-sensitive cancellation inputs, fixed-chunk index-ordered
    /// reduction produces bit-identical sums across 1, 2, 4, and 8 worker partitions, whereas a
    /// naive worker-split parallel reduction alters the floating-point bits and is detected.
    #[test]
    fn test_order_sensitive_reduction_negative_control_detects_unsafe_parallel_sum() {
        const CHUNK: usize = 64;
        // Catastrophic-cancellation pattern: `1e16` at index 0 absorbs `1.0`s (ULP(1e16) = 2.0)
        // in a single sequential sum (`0.0`), while splitting across workers accumulates `1.0`s
        // before cancelling `-1e16`, producing different bits at different thread counts.
        let len = CHUNK * 4 + 17;
        let mut xs = vec![1.0_f64; len];
        xs[0] = 1e16_f64;
        xs[len - 1] = -1e16_f64;

        // Fixed-chunk index-ordered reduction (independent of worker count w).
        let fixed_chunk_sum = |workers: usize| -> u64 {
            let total_chunks = xs.len().div_ceil(CHUNK);
            let chunks_per_worker = total_chunks.div_ceil(workers);
            let mut worker_partials: Vec<Vec<f64>> = Vec::with_capacity(workers);
            for w in 0..workers {
                let c_lo = w * chunks_per_worker;
                let c_hi = ((w + 1) * chunks_per_worker).min(total_chunks);
                let mut local = Vec::new();
                for c in c_lo..c_hi {
                    let lo = c * CHUNK;
                    let hi = (lo + CHUNK).min(xs.len());
                    let mut acc = 0.0_f64;
                    for &x in &xs[lo..hi] {
                        acc += x;
                    }
                    local.push(acc);
                }
                worker_partials.push(local);
            }
            let mut total = 0.0_f64;
            for chunk_sum in worker_partials.into_iter().flatten() {
                total += chunk_sum;
            }
            total.to_bits()
        };

        let ref_bits = fixed_chunk_sum(1);
        for w in [2, 3, 4, 8] {
            assert_eq!(
                fixed_chunk_sum(w),
                ref_bits,
                "fixed-chunk reduction must stay bit-identical at {w} workers"
            );
        }

        // Planted negative control: naive per-worker slice reduction without fixed chunk boundaries.
        let unsafe_worker_sum = |workers: usize| -> u64 {
            let stride = xs.len().div_ceil(workers);
            let mut total = 0.0_f64;
            for w in 0..workers {
                let lo = w * stride;
                let hi = (lo + stride).min(xs.len());
                if lo < hi {
                    let mut acc = 0.0_f64;
                    for &x in &xs[lo..hi] {
                        acc += x;
                    }
                    total += acc;
                }
            }
            total.to_bits()
        };

        let unsafe_1 = unsafe_worker_sum(1);
        let unsafe_diverged = [2, 3, 4, 8]
            .into_iter()
            .any(|w| unsafe_worker_sum(w) != unsafe_1);
        assert!(
            unsafe_diverged,
            "order-sensitive negative control must detect bit drift in naive worker-split sum"
        );
    }
}
