//! Screen a directory of cited worksheets through the existing BD-350 readers.
//! Determinism: Exact

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::sources::load_issuer_profile;
use fmine_registry::sources::is_date;
use fmine_registry::table_identities::TABLE_IDENTITIES_SCHEMA;
use fmine_registry::toml_subset::{TomlValue, parse_subset};

use crate::capital::{drill_release, npv_replay};
use crate::{CliCommand, Envelope, json_string as js, tables, worksheet_input};

pub(super) const VERB: &str = "screen target";
pub(super) const USAGE: &str = "screen target --docs <directory> [--table-as-of YYYY-MM-DD]";
const BAD_DATE: &str = "screen target --table-as-of requires a valid YYYY-MM-DD date";
const MAX_DOCUMENTS: usize = 64;
const MAX_BYTES: u64 = 65_536;
const NO_CLAIM: &str = "Findings apply to the submitted worksheets and their declared inputs. This review supplies no issuer authentication, mineral classification, investment authorization or regulatory approval.";

pub(super) fn parse(words: &[String]) -> Result<CliCommand, String> {
    let usage = || format!("expected 'fmine {USAGE}'");
    let [screen, target, args @ ..] = words else {
        return Err(usage());
    };
    if screen != "screen" || target != "target" {
        return Err(usage());
    }
    let (mut docs, mut table_as_of) = (None, None);
    let mut args = args.iter();
    while let Some(option) = args.next() {
        let value = args
            .next()
            .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
            .ok_or_else(usage)?;
        match option.as_str() {
            "--docs" if docs.is_none() => docs = Some(PathBuf::from(value)),
            "--table-as-of" if table_as_of.is_none() => {
                if !is_date(value) {
                    return Err(BAD_DATE.into());
                }
                table_as_of = Some(value.clone());
            }
            _ => return Err(usage()),
        }
    }
    Ok(CliCommand::ScreenTarget {
        docs: docs.ok_or_else(usage)?,
        table_as_of,
    })
}

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

fn inspect(
    root: &Path,
    path: &Path,
    table_as_of: Option<&str>,
) -> Result<(String, Envelope), Refusal> {
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
                Some(TomlValue::Str(schema)) if schema == TABLE_IDENTITIES_SCHEMA => {
                    let result = match doc.top_get("issuer").map(|field| &field.value) {
                        Some(TomlValue::Str(issuer)) => {
                            load_issuer_profile(root, issuer).and_then(|profile| {
                                tables::evaluate_definitions(
                                    root,
                                    &profile,
                                    path,
                                    &text,
                                    table_as_of,
                                    None,
                                )
                            })
                        }
                        _ => Err(invalid(
                            "table definitions require a quoted issuer registry name".into(),
                        )),
                    };
                    Ok(("check tables", result))
                }
                _ => Err(invalid(
                    "expected a drill-release, developer NPV or cited table worksheet schema"
                        .into(),
                )),
            },
        );
    let (verb, result) = result.unwrap_or_else(|refusal| (VERB, Err(refusal)));
    Ok((
        provenance,
        result.unwrap_or_else(|refusal| Envelope::refused(verb, refusal, Vec::new())),
    ))
}

pub(super) fn execute(docs: &Path, root: &Path, table_as_of: Option<&str>) -> Envelope {
    if table_as_of.is_some_and(|date| !is_date(date)) {
        return Envelope::usage_error(VERB, BAD_DATE);
    }
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
        let (source, result) = inspect(root, &path, table_as_of).unwrap_or_else(|refusal| {
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
    let cut_json =
        table_as_of.map_or_else(String::new, |date| format!("\"table_as_of\":{},", js(date)));
    let cut_md = table_as_of.map_or_else(String::new, |date| {
        format!("\n\nTable publication cut `{date}` applies to registered table documents only.")
    });
    envelope.payload_json = format!(
        "{{\"schema\":\"fmine.target-screen.v1\",{cut_json}\"screened\":{screened},\"refused\":{refused},\"documents\":[{}]}}",
        reports.join(","),
    );
    envelope.rendered_markdown = Some(format!(
        "# Target screening\n\n{screened} of {total} worksheets screened. {refused} refused.{cut_md}\n\n{NO_CLAIM}\n{details}"
    ));
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
