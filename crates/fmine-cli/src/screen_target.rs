//! Screen a directory of cited worksheets through the existing BD-350 readers.
//! Determinism: Exact

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlValue, parse_subset};

use crate::capital::{drill_release, npv_replay};
use crate::{Envelope, json_string as js, worksheet_input};

pub(super) const VERB: &str = "screen target";
pub(super) const USAGE: &str = "screen target --docs <directory>";
const MAX_DOCUMENTS: usize = 64;
const MAX_BYTES: u64 = 65_536;
const NO_CLAIM: &str = "Findings apply to the submitted worksheets and their declared inputs. This review supplies no issuer authentication, mineral classification, investment authorization or regulatory approval.";

fn unavailable(path: &Path, detail: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("target screening input {}: {detail}", path.display()),
    )
}

fn pack(directory: &Path) -> Result<Vec<PathBuf>, Refusal> {
    let candidates = std::fs::read_dir(directory).map_err(|error| unavailable(directory, error))?;
    let mut files = Vec::new();
    for entry in candidates {
        let path = entry.map_err(|error| unavailable(directory, error))?.path();
        if path.extension() == Some(OsStr::new("toml")) {
            files.push(path);
            if files.len() > MAX_DOCUMENTS {
                return Err(unavailable(
                    directory,
                    "at most 64 TOML worksheets are supported",
                ));
            }
        }
    }
    if files.is_empty() {
        return Err(unavailable(directory, "no TOML worksheets found"));
    }
    files.sort_unstable();
    Ok(files)
}

fn inspect(path: &Path) -> Result<(String, Envelope), Refusal> {
    if !path
        .symlink_metadata()
        .map_err(|error| unavailable(path, error))?
        .is_file()
    {
        return Err(unavailable(path, "expected a regular TOML worksheet"));
    }
    let (text, source) = worksheet_input::read(path, "target screening worksheet", MAX_BYTES)?;
    let provenance = worksheet_input::source_json(&source, text.len());
    let invalid = |detail| Refusal::new(RefusalCode::BasisMismatch105, detail);
    let result = parse_subset(&text)
        .map_err(|error| invalid(error.to_string()))
        .and_then(
            |doc| match doc.top_get("schema").map(|field| &field.value) {
                Some(TomlValue::Str(schema)) if schema == drill_release::SCHEMA => Ok((
                    drill_release::VERB,
                    drill_release::evaluate_document(&doc, &source, text.len()),
                )),
                Some(TomlValue::Str(schema)) if schema == npv_replay::SCHEMA => Ok((
                    npv_replay::VERB,
                    npv_replay::evaluate_document(&doc, &source, text.len()),
                )),
                _ => Err(invalid(
                    "expected a drill-release or developer NPV worksheet schema".into(),
                )),
            },
        );
    let (verb, result) = result.unwrap_or_else(|refusal| (VERB, Err(refusal)));
    Ok((
        provenance,
        result.unwrap_or_else(|refusal| Envelope::refused(verb, refusal, Vec::new())),
    ))
}

pub(super) fn execute(docs: &Path, root: &Path) -> Envelope {
    let directory = root.join(docs);
    let files = match pack(&directory) {
        Ok(files) => files,
        Err(refusal) => return Envelope::refused(VERB, refusal, Vec::new()),
    };
    let mut refused = 0;
    let mut first = None;
    let mut reports = Vec::with_capacity(files.len());
    let mut details = String::new();
    for path in files {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        let (source, result) = inspect(&path).unwrap_or_else(|refusal| {
            ("null".into(), Envelope::refused(VERB, refusal, Vec::new()))
        });
        let _ = writeln!(details, "\n## {}\n", worksheet_input::markdown_cell(&name));
        if let Some(refusal) = &result.refusal {
            refused += 1;
            first.get_or_insert_with(|| {
                let mut r = refusal.clone();
                r.what = format!("{name}: {}", r.what);
                r
            });
            if result.rendered_markdown.is_none() {
                let _ = writeln!(
                    details,
                    "Refused `{}`. {}\n",
                    refusal.code,
                    worksheet_input::markdown_cell(&refusal.what)
                );
            }
        }
        if let Some(markdown) = &result.rendered_markdown {
            let _ = writeln!(
                details,
                "### {}",
                markdown.strip_prefix("# ").unwrap_or(markdown)
            );
        }
        reports.push(format!(
            "{{\"file\":{},\"source\":{source},\"result\":{}}}",
            js(&name),
            result.to_json()
        ));
    }
    let total = reports.len();
    let screened = total - refused;
    let mut envelope = match first {
        Some(refusal) => Envelope::refused(VERB, refusal, Vec::new()),
        None => Envelope::ok(VERB, String::new(), Vec::new()),
    };
    envelope.payload_json = format!(
        "{{\"schema\":\"fmine.target-screen.v1\",\"screened\":{screened},\"refused\":{refused},\"documents\":[{}]}}",
        reports.join(","),
    );
    envelope.rendered_markdown = Some(format!(
        "# Target screening\n\n{screened} of {total} worksheets screened. {refused} refused.\n\n{NO_CLAIM}\n{details}"
    ));
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
