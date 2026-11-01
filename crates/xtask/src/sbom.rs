//! `sbom`: a CycloneDX 1.5 bill of materials for the `fmine` binary (BD-073, the M0 security pack of plan §39;
//! GATE-DATA-02).
//!
//! The components are `fmine-cli`'s normal and build closure over all targets, as `cargo tree --locked` resolves
//! it, so dev-dependencies and `xtask` stay out. Cargo supplies each licence, and `Cargo.lock` supplies each
//! source and checksum. The output is a pure function of the tree, the lock, the pinned toolchain and the optional
//! binary. It has no timestamp or serial number, its components and edges are sorted, and its paths are relative to
//! the workspace, so one commit always gives the same bytes. A source that cannot be described is refused, never
//! guessed: a registry other than crates.io, a path outside the workspace, a git source without a locked commit, or
//! a package the lock does not hold exactly once.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use fmine_core::hash::sha256;

use crate::int_as::parse_tree_line;
use crate::strip_kv_str;

/// The package whose binary the bill describes.
pub const ROOT_PACKAGE: &str = "fmine-cli";
/// `cargo tree`'s arguments for the closure; the bill records them.
pub const TREE_ARGS: [&str; 9] = [
    "tree",
    "--locked",
    "-e",
    "normal,build",
    "--target",
    "all",
    "--prefix",
    "depth",
    "-p",
];
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// One `[[package]]` of `Cargo.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    /// `None` for a path package.
    pub source: Option<String>,
    pub checksum: Option<String>,
}

/// The `[[package]]` rows of a `Cargo.lock`, in file order.
#[must_use]
pub fn parse_lock(text: &str) -> Vec<LockPackage> {
    let mut out: Vec<LockPackage> = Vec::new();
    let mut in_package = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && !line.starts_with("[\"") {
            in_package = line == "[[package]]";
            if in_package {
                out.push(LockPackage {
                    name: String::new(),
                    version: String::new(),
                    source: None,
                    checksum: None,
                });
            }
            continue;
        }
        let Some(pkg) = out.last_mut().filter(|_| in_package) else {
            continue;
        };
        if let Some(v) = strip_kv_str(line, "name") {
            pkg.name = v;
        } else if let Some(v) = strip_kv_str(line, "version") {
            pkg.version = v;
        } else if let Some(v) = strip_kv_str(line, "source") {
            pkg.source = Some(v);
        } else if let Some(v) = strip_kv_str(line, "checksum") {
            pkg.checksum = Some(v);
        }
    }
    out
}

/// Where a component comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A path package, relative to the workspace root.
    Path(String),
    /// A crates.io package and its `.crate` sha256.
    CratesIo { checksum: String },
    /// A git package: the repository URL without query or fragment, and the locked commit.
    Git { url: String, commit: String },
}

/// One component of the bill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub bom_ref: String,
    pub name: String,
    pub version: String,
    /// The SPDX expression cargo reports; empty when the manifest names none.
    pub license: String,
    pub origin: Origin,
}

impl Component {
    /// `pkg:cargo/…` for crates.io and git packages. A path package has none: `pkg:cargo/<name>` would name a
    /// crates.io crate, which may be someone else's.
    #[must_use]
    pub fn purl(&self) -> Option<String> {
        let base = format!("pkg:cargo/{}@{}", self.name, self.version);
        match &self.origin {
            Origin::Path(_) => None,
            Origin::CratesIo { .. } => Some(base),
            Origin::Git { url, commit } => Some(format!(
                "{base}?vcs_url={}",
                percent_encode(&format!("git+{url}@{commit}"))
            )),
        }
    }
}

/// The bill before rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bom {
    pub root: Component,
    /// Every other closure package, sorted by `bom_ref`.
    pub components: Vec<Component>,
    /// `bom_ref` → the `bom_ref`s it depends on, for every component and the root.
    pub edges: BTreeMap<String, BTreeSet<String>>,
}

/// What the bill records besides the closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// rust-toolchain.toml's `channel`.
    pub toolchain: String,
    pub lock_sha256: String,
    /// The binary's file name and sha256, when one was given.
    pub binary: Option<(String, String)>,
}

/// The bill for the closure in `tree` (`cargo tree` lines formatted `|{l}|{p}`, depth-prefixed) against `lock`.
/// `root_dir` is the workspace root as it appears in the tree's path sources.
pub fn build(tree: &str, lock: &[LockPackage], root_dir: &str) -> Result<Bom, Vec<String>> {
    let mut errors = Vec::new();
    let mut by_key: BTreeMap<(&str, &str), Vec<&LockPackage>> = BTreeMap::new();
    for pkg in lock {
        by_key
            .entry((pkg.name.as_str(), pkg.version.as_str()))
            .or_default()
            .push(pkg);
    }
    let prefix = format!("{}/", root_dir.trim_end_matches('/'));
    let mut seen: BTreeMap<String, Component> = BTreeMap::new();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut root: Option<String> = None;
    for (n, line) in tree.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed = line.split_once('|').and_then(|(depth, rest)| {
            let (license, package) = rest.split_once('|')?;
            Some((parse_tree_line(&format!("{depth}{package}"))?, license))
        });
        let Some((pkg, license)) = parsed else {
            errors.push(format!("cargo tree line {}: cannot read {line:?}", n + 1));
            continue;
        };
        if pkg.depth > stack.len() {
            errors.push(format!(
                "cargo tree line {}: depth {} under {} open levels",
                n + 1,
                pkg.depth,
                stack.len()
            ));
            continue;
        }
        let origin = match by_key
            .get(&(pkg.name.as_str(), pkg.version.as_str()))
            .map(Vec::as_slice)
        {
            Some([one]) => origin(one, &pkg.source, &prefix),
            Some([]) | None => Err(format!(
                "{} {} is in the tree but not in Cargo.lock",
                pkg.name, pkg.version
            )),
            Some(many) => Err(format!(
                "Cargo.lock holds {} {} {} times; the bill cannot tell them apart",
                pkg.name,
                pkg.version,
                many.len()
            )),
        };
        let origin = match origin {
            Ok(origin) => origin,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let component = Component {
            bom_ref: String::new(),
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            license: license.trim().to_string(),
            origin,
        };
        let bom_ref = component.purl().unwrap_or_else(|| match &component.origin {
            Origin::Path(rel) => format!("path+{rel}#{}@{}", component.name, component.version),
            _ => String::new(),
        });
        stack.truncate(pkg.depth);
        if let Some(parent) = stack.last() {
            edges
                .entry(parent.clone())
                .or_default()
                .insert(bom_ref.clone());
        } else if let Some(first) = &root {
            errors.push(format!(
                "cargo tree has a second root {bom_ref} after {first}"
            ));
        } else if pkg.name != ROOT_PACKAGE {
            errors.push(format!(
                "cargo tree's root is {}, not {ROOT_PACKAGE}",
                pkg.name
            ));
        } else {
            root = Some(bom_ref.clone());
        }
        edges.entry(bom_ref.clone()).or_default();
        stack.push(bom_ref.clone());
        seen.entry(bom_ref.clone()).or_insert(Component {
            bom_ref,
            ..component
        });
    }
    let root = root.and_then(|r| seen.remove(&r));
    if root.is_none() && errors.is_empty() {
        errors.push(format!("cargo tree lists no {ROOT_PACKAGE}"));
    }
    match root {
        Some(root) if errors.is_empty() => Ok(Bom {
            root,
            components: seen.into_values().collect(),
            edges,
        }),
        _ => Err(errors),
    }
}

/// A lock row's origin, checked against the source `cargo tree` printed for it.
fn origin(pkg: &LockPackage, tree_source: &str, prefix: &str) -> Result<Origin, String> {
    let id = format!("{} {}", pkg.name, pkg.version);
    match pkg.source.as_deref() {
        None => tree_source
            .strip_prefix(prefix)
            .filter(|rel| !rel.is_empty())
            .map(|rel| Origin::Path(rel.to_string()))
            .ok_or_else(|| format!("{id} is a path package outside the workspace: {tree_source}")),
        Some(CRATES_IO) => pkg
            .checksum
            .clone()
            .map(|checksum| Origin::CratesIo { checksum })
            .ok_or_else(|| format!("{id} is from crates.io but Cargo.lock has no checksum")),
        Some(git) if git.starts_with("git+") => {
            let rest = git.strip_prefix("git+").unwrap_or(git);
            let Some((head, commit)) = rest.split_once('#') else {
                return Err(format!("{id}: git source {git} has no locked commit"));
            };
            let url = head.split_once('?').map_or(head, |(url, _)| url);
            Ok(Origin::Git {
                url: url.to_string(),
                commit: commit.to_string(),
            })
        }
        Some(other) => Err(format!(
            "{id}: source {other} is not crates.io, git or a path; the bill does not describe it yet"
        )),
    }
}

/// Every byte but ASCII letters, digits and `-._~:/` as `%XX`.
fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~:/".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A JSON value with object keys in the order given.
enum Json {
    Str(String),
    Int(u64),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

fn s(text: &str) -> Json {
    Json::Str(text.to_string())
}

fn property(name: &str, value: &str) -> Json {
    Json::Obj(vec![("name", s(name)), ("value", s(value))])
}

impl Json {
    fn write(&self, out: &mut String, indent: usize) {
        let pad = |out: &mut String, n: usize| out.push_str(&"  ".repeat(n));
        match self {
            Json::Str(text) => {
                out.push('"');
                for c in text.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if u32::from(c) < 0x20 => {
                            out.push_str(&format!("\\u{:04x}", u32::from(c)))
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Json::Int(n) => out.push_str(&n.to_string()),
            Json::Arr(items) if items.is_empty() => out.push_str("[]"),
            Json::Obj(fields) if fields.is_empty() => out.push_str("{}"),
            Json::Arr(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    pad(out, indent + 1);
                    item.write(out, indent + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                pad(out, indent);
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push_str("{\n");
                for (i, (key, value)) in fields.iter().enumerate() {
                    pad(out, indent + 1);
                    Json::Str((*key).to_string()).write(out, indent + 1);
                    out.push_str(": ");
                    value.write(out, indent + 1);
                    out.push_str(if i + 1 < fields.len() { ",\n" } else { "\n" });
                }
                pad(out, indent);
                out.push('}');
            }
        }
    }
}

fn component_json(c: &Component, kind: &str, extra: Vec<Json>, mut hashes: Vec<Json>) -> Json {
    let mut fields = vec![
        ("type", s(kind)),
        ("bom-ref", s(&c.bom_ref)),
        ("name", s(&c.name)),
        ("version", s(&c.version)),
    ];
    if let Some(purl) = c.purl() {
        fields.push(("purl", s(&purl)));
    }
    if !c.license.is_empty() {
        fields.push((
            "licenses",
            Json::Arr(vec![Json::Obj(vec![("expression", s(&c.license))])]),
        ));
    }
    let mut properties = Vec::new();
    match &c.origin {
        Origin::Path(rel) => properties.push(property("cargo:path", rel)),
        Origin::CratesIo { checksum } => {
            hashes.push(Json::Obj(vec![
                ("alg", s("SHA-256")),
                ("content", s(checksum)),
            ]));
            properties.push(property("cargo:source", CRATES_IO));
        }
        Origin::Git { url, commit } => {
            properties.push(property("cargo:source", &format!("git+{url}#{commit}")));
        }
    }
    if !hashes.is_empty() {
        fields.push(("hashes", Json::Arr(hashes)));
    }
    properties.extend(extra);
    fields.push(("properties", Json::Arr(properties)));
    Json::Obj(fields)
}

/// The bill as CycloneDX 1.5 JSON, newline-terminated.
#[must_use]
pub fn render(bom: &Bom, ctx: &Context) -> String {
    let (root_extra, root_hashes) = match &ctx.binary {
        Some((file, digest)) => (
            vec![property("cargo:binary", file)],
            vec![Json::Obj(vec![
                ("alg", s("SHA-256")),
                ("content", s(digest)),
            ])],
        ),
        None => (Vec::new(), Vec::new()),
    };
    let tree_command = format!(
        "cargo {} {ROOT_PACKAGE} --format |{{l}}|{{p}}",
        TREE_ARGS.join(" ")
    );
    let doc = Json::Obj(vec![
        ("bomFormat", s("CycloneDX")),
        ("specVersion", s("1.5")),
        ("version", Json::Int(1)),
        (
            "metadata",
            Json::Obj(vec![
                (
                    "tools",
                    Json::Obj(vec![(
                        "components",
                        Json::Arr(vec![Json::Obj(vec![
                            ("type", s("application")),
                            ("name", s("xtask sbom")),
                            ("version", s(env!("CARGO_PKG_VERSION"))),
                        ])]),
                    )]),
                ),
                (
                    "component",
                    component_json(&bom.root, "application", root_extra, root_hashes),
                ),
                (
                    "properties",
                    Json::Arr(vec![
                        property("fmine:rust-toolchain", &ctx.toolchain),
                        property("fmine:cargo-lock-sha256", &ctx.lock_sha256),
                        property("fmine:closure", &tree_command),
                    ]),
                ),
            ]),
        ),
        (
            "components",
            Json::Arr(
                bom.components
                    .iter()
                    .map(|c| component_json(c, "library", Vec::new(), Vec::new()))
                    .collect(),
            ),
        ),
        (
            "dependencies",
            Json::Arr(
                bom.edges
                    .iter()
                    .map(|(from, to)| {
                        Json::Obj(vec![
                            ("ref", s(from)),
                            ("dependsOn", Json::Arr(to.iter().map(|t| s(t)).collect())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    let mut out = String::new();
    doc.write(&mut out, 0);
    out.push('\n');
    out
}

/// `fmine-cli`'s closure as `cargo tree` prints it for [`build`]. Not [`crate::int_as::cargo_tree`]: the licence
/// needs its own `--format`.
fn closure_tree(root: &Path) -> Result<String, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(root)
        .args(TREE_ARGS)
        .args([ROOT_PACKAGE, "--format", "|{l}|{p}"])
        .output()
        .map_err(|e| format!("cargo tree: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo tree -p {ROOT_PACKAGE}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("cargo tree -p {ROOT_PACKAGE}: {e}"))
}

/// rust-toolchain.toml's `channel`.
fn toolchain_channel(text: &str) -> Option<String> {
    text.lines()
        .map(|line| line.split_once('#').map_or(line, |(code, _)| code))
        .find_map(|line| strip_kv_str(line, "channel"))
        .filter(|c| !c.is_empty())
}

/// The workspace's bill, with the binary at `binary` hashed into the root component when given.
pub fn sbom(root: &Path, binary: Option<&Path>) -> Result<String, Vec<String>> {
    let read = |name: &str| fs::read(root.join(name)).map_err(|e| vec![format!("{name}: {e}")]);
    let lock_bytes = read("Cargo.lock")?;
    let toolchain = toolchain_channel(&String::from_utf8_lossy(&read("rust-toolchain.toml")?))
        .ok_or_else(|| vec!["rust-toolchain.toml has no `channel`".to_string()])?;
    let binary = match binary {
        None => None,
        Some(path) => {
            let bytes = fs::read(path).map_err(|e| vec![format!("{}: {e}", path.display())])?;
            let file = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .ok_or_else(|| vec![format!("{} names no file", path.display())])?;
            Some((file, sha256(&bytes).to_hex()))
        }
    };
    let tree = closure_tree(root).map_err(|e| vec![e])?;
    let lock = parse_lock(&String::from_utf8_lossy(&lock_bytes));
    let canonical = fs::canonicalize(root).map_err(|e| vec![format!("{}: {e}", root.display())])?;
    let bom = build(&tree, &lock, &canonical.to_string_lossy())?;
    Ok(render(
        &bom,
        &Context {
            toolchain,
            lock_sha256: sha256(&lock_bytes).to_hex(),
            binary,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = "# This file is automatically @generated by Cargo.\nversion = 4\n\n\
        [[package]]\nname = \"fmine-cli\"\nversion = \"0.1.0\"\ndependencies = [\n \"lib-a\",\n \"serde\",\n]\n\n\
        [[package]]\nname = \"lib-a\"\nversion = \"0.2.0\"\ndependencies = [\n \"gitdep\",\n \"serde\",\n]\n\n\
        [[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n\
        source = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"abc123\"\n\n\
        [[package]]\nname = \"gitdep\"\nversion = \"0.3.0\"\n\
        source = \"git+https://github.com/Dicklesworthstone/gitdep?rev=78b6#78b64636ff\"\n";

    const TREE: &str = "0|MIT|fmine-cli v0.1.0 (/w/crates/fmine-cli)\n\
        1|MIT|lib-a v0.2.0 (/w/crates/lib-a)\n\
        2|Apache-2.0|gitdep v0.3.0 (https://github.com/Dicklesworthstone/gitdep?rev=78b6#78b64636)\n\
        2|MIT OR Apache-2.0|serde v1.0.0\n\
        1|MIT OR Apache-2.0|serde v1.0.0 (*)\n";

    fn ctx() -> Context {
        Context {
            toolchain: "nightly-2026-08-31".to_string(),
            lock_sha256: "00ff".to_string(),
            binary: Some(("fmine".to_string(), "beef".to_string())),
        }
    }

    #[test]
    fn closure_sources_and_edges_come_from_tree_and_lock() {
        let bom = build(TREE, &parse_lock(LOCK), "/w").expect("synthetic closure builds");
        assert_eq!(bom.root.bom_ref, "path+crates/fmine-cli#fmine-cli@0.1.0");
        let refs: Vec<&str> = bom.components.iter().map(|c| c.bom_ref.as_str()).collect();
        let git = "pkg:cargo/gitdep@0.3.0?vcs_url=git%2Bhttps://github.com/Dicklesworthstone/gitdep%4078b64636ff";
        assert_eq!(
            refs,
            vec![
                "path+crates/lib-a#lib-a@0.2.0",
                git,
                "pkg:cargo/serde@1.0.0",
            ]
        );
        let serde = &bom.components[2];
        assert_eq!(
            serde.origin,
            Origin::CratesIo {
                checksum: "abc123".to_string()
            }
        );
        assert_eq!(serde.license, "MIT OR Apache-2.0");
        // The git commit is the lock's full one, not the tree's short print.
        assert_eq!(
            bom.components[1].origin,
            Origin::Git {
                url: "https://github.com/Dicklesworthstone/gitdep".to_string(),
                commit: "78b64636ff".to_string()
            }
        );
        let edges: Vec<(String, Vec<String>)> = bom
            .edges
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
            .collect();
        assert_eq!(
            edges,
            vec![
                (
                    "path+crates/fmine-cli#fmine-cli@0.1.0".to_string(),
                    vec![
                        "path+crates/lib-a#lib-a@0.2.0".to_string(),
                        "pkg:cargo/serde@1.0.0".to_string()
                    ]
                ),
                (
                    "path+crates/lib-a#lib-a@0.2.0".to_string(),
                    vec![git.to_string(), "pkg:cargo/serde@1.0.0".to_string()]
                ),
                (git.to_string(), vec![]),
                ("pkg:cargo/serde@1.0.0".to_string(), vec![]),
            ]
        );
    }

    #[test]
    fn rendering_is_cyclonedx_and_independent_of_lock_order() {
        let mut lock = parse_lock(LOCK);
        let first = render(&build(TREE, &lock, "/w").expect("builds"), &ctx());
        lock.reverse();
        let again = render(&build(TREE, &lock, "/w/").expect("builds"), &ctx());
        assert_eq!(first, again);
        for needle in [
            "  \"bomFormat\": \"CycloneDX\",\n  \"specVersion\": \"1.5\",\n  \"version\": 1,\n",
            "\"hashes\": [\n        {\n          \"alg\": \"SHA-256\",\n          \"content\": \"beef\"\n",
            "\"name\": \"cargo:binary\",\n          \"value\": \"fmine\"",
            "\"name\": \"fmine:rust-toolchain\",\n        \"value\": \"nightly-2026-08-31\"",
            "\"licenses\": [\n        {\n          \"expression\": \"MIT OR Apache-2.0\"\n",
            "\"content\": \"abc123\"",
            "\"ref\": \"pkg:cargo/serde@1.0.0\",\n      \"dependsOn\": []\n",
        ] {
            assert!(first.contains(needle), "missing {needle:?} in\n{first}");
        }
        assert!(!first.contains("timestamp") && !first.contains("serialNumber"));
        assert!(first.ends_with("}\n"));
    }

    #[test]
    fn json_strings_are_escaped() {
        let mut out = String::new();
        s("a\"b\\c\nd\u{1}é").write(&mut out, 0);
        assert_eq!(out, "\"a\\\"b\\\\c\\nd\\u0001é\"");
    }

    #[test]
    fn undescribable_closures_are_refused() {
        let lock = parse_lock(LOCK);
        let other_registry = LOCK.replace(
            "registry+https://github.com/rust-lang/crates.io-index",
            "sparse+https://example.org/index/",
        );
        let twice = format!("{LOCK}\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n");
        let no_commit = LOCK.replace("?rev=78b6#78b64636ff", "?rev=78b6");
        let no_checksum = LOCK.replace("checksum = \"abc123\"\n", "");
        let rows: [(&str, String, Vec<LockPackage>, &str); 9] = [
            (
                "not in the lock",
                TREE.replace("lib-a v0.2.0", "lib-b v0.2.0"),
                lock.clone(),
                "not in Cargo.lock",
            ),
            ("ambiguous", TREE.to_string(), parse_lock(&twice), "2 times"),
            (
                "path outside",
                TREE.replace("/w/crates/lib-a", "/elsewhere/lib-a"),
                lock.clone(),
                "outside the workspace",
            ),
            (
                "other registry",
                TREE.to_string(),
                parse_lock(&other_registry),
                "not crates.io",
            ),
            (
                "git without commit",
                TREE.to_string(),
                parse_lock(&no_commit),
                "no locked commit",
            ),
            (
                "registry without checksum",
                TREE.to_string(),
                parse_lock(&no_checksum),
                "no checksum",
            ),
            (
                "wrong root",
                TREE.replace("fmine-cli v0.1.0", "lib-a v0.2.0"),
                lock.clone(),
                "not fmine-cli",
            ),
            (
                "second root",
                format!("{TREE}0|MIT|lib-a v0.2.0 (/w/crates/lib-a)\n"),
                lock.clone(),
                "second root",
            ),
            (
                "depth jump",
                TREE.replace("1|MIT|lib-a", "3|MIT|lib-a"),
                lock.clone(),
                "depth 3",
            ),
        ];
        for (what, tree, lock, expect) in rows {
            let errors = build(&tree, &lock, "/w").expect_err(what);
            assert!(
                errors.iter().any(|e| e.contains(expect)),
                "{what}: {errors:?} lacks {expect:?}"
            );
        }
    }

    #[test]
    fn toolchain_channel_ignores_comments() {
        let text =
            "[toolchain]\n# channel = \"stable\"\nchannel = \"nightly-2026-08-31\" # pinned\n";
        assert_eq!(
            toolchain_channel(text).as_deref(),
            Some("nightly-2026-08-31")
        );
        assert_eq!(toolchain_channel("[toolchain]\n"), None);
    }

    /// The real workspace: the bill lists exactly the packages reachable from fmine-cli through normal and build
    /// dependencies in the member manifests, read without cargo.
    #[test]
    fn the_workspace_bill_is_the_manifest_closure() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask sits two levels below the workspace root");
        let json = sbom(root, None).expect("the workspace bill builds");
        let lock = parse_lock(&fs::read_to_string(root.join("Cargo.lock")).expect("lock"));
        let tree = closure_tree(root).expect("cargo tree runs");
        let canonical = fs::canonicalize(root).expect("root");
        let bom = build(&tree, &lock, &canonical.to_string_lossy()).expect("builds");

        let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for manifest in crate::workspace_member_manifests(root).expect("members") {
            let (name, normal, _dev) = crate::parse_cargo_manifest(&manifest).expect("manifest");
            deps.insert(name, normal);
        }
        let mut expect = BTreeSet::new();
        let mut todo = vec![ROOT_PACKAGE.to_string()];
        while let Some(name) = todo.pop() {
            if expect.insert(name.clone()) {
                todo.extend(deps.get(&name).cloned().unwrap_or_default());
            }
        }
        let mut listed: Vec<&str> = bom.components.iter().map(|c| c.name.as_str()).collect();
        listed.push(bom.root.name.as_str());
        listed.sort_unstable();
        let unique: BTreeSet<&str> = listed.iter().copied().collect();
        assert_eq!(
            listed.len(),
            unique.len(),
            "a package is listed twice: {listed:?}"
        );
        assert_eq!(
            unique,
            expect.iter().map(String::as_str).collect::<BTreeSet<_>>()
        );
        assert!(!unique.contains("xtask"));
        assert_eq!(bom.edges.len(), unique.len());
        // Each package's edges are its manifest's normal and build dependencies.
        let name_of: BTreeMap<&str, &str> = bom
            .components
            .iter()
            .chain([&bom.root])
            .map(|c| (c.bom_ref.as_str(), c.name.as_str()))
            .collect();
        for (from, to) in &bom.edges {
            let got: BTreeSet<&str> = to.iter().map(|t| name_of[t.as_str()]).collect();
            let name = name_of[from.as_str()];
            let want: BTreeSet<&str> = deps[name].iter().map(String::as_str).collect();
            assert_eq!(got, want, "edges of {name}");
        }
        for c in &bom.components {
            assert_eq!(c.license, "MIT", "{}", c.name);
            assert!(json.contains(&format!("\"bom-ref\": \"{}\"", c.bom_ref)));
        }
        assert!(
            !json.contains(&*canonical.to_string_lossy()),
            "an absolute path leaked"
        );
    }
}
