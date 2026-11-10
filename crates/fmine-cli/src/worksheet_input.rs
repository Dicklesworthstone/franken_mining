//! Bounded worksheet reads retain the exact submitted bytes and their source identity.

use std::io::Read as _;
use std::path::Path;

use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};

pub(super) fn read(
    path: &Path,
    kind: &str,
    max_bytes: u64,
) -> Result<(String, SourceAnchor), Refusal> {
    read_source(path, kind, max_bytes, None)
}

/// Binary sources use the same regular-file and byte bounds without a UTF-8 requirement.
pub(super) fn read_bytes(
    path: &Path,
    kind: &str,
    max_bytes: u64,
) -> Result<(Vec<u8>, SourceAnchor), Refusal> {
    read_source_bytes(path, kind, max_bytes, None, false)
}

/// Decision and capital models retain their established source locators and input-error text.
pub(super) fn read_model(
    path: &Path,
    kind: &str,
    locator: &str,
) -> Result<(String, SourceAnchor), Refusal> {
    read_source(path, kind, 4 * 1024 * 1024, Some(locator))
}

fn read_source(
    path: &Path,
    kind: &str,
    max_bytes: u64,
    model_locator: Option<&str>,
) -> Result<(String, SourceAnchor), Refusal> {
    let (bytes, source) = read_source_bytes(path, kind, max_bytes, model_locator, true)?;
    let text = String::from_utf8(bytes).map_err(|e| {
        let input = if model_locator.is_some() {
            ""
        } else {
            " input"
        };
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{kind}{input} {}: {e}", path.display()),
        )
    })?;
    Ok((text, source))
}

fn read_source_bytes(
    path: &Path,
    kind: &str,
    max_bytes: u64,
    model_locator: Option<&str>,
    utf8: bool,
) -> Result<(Vec<u8>, SourceAnchor), Refusal> {
    let (input, path_error, encoding, noun) = if model_locator.is_some() {
        ("", "source path must be UTF-8", "", "model")
    } else {
        (" input", "path must be UTF-8", " UTF-8", "input")
    };
    let encoding = if utf8 { encoding } else { "" };
    let fail = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{kind}{input} {}: {why}", path.display()),
        )
    };
    let limit = if max_bytes.is_multiple_of(1024 * 1024) {
        format!("{} MiB", max_bytes / (1024 * 1024))
    } else {
        format!("{max_bytes} bytes")
    };
    let name = path.to_str().ok_or_else(|| fail(path_error.into()))?;
    let metadata = std::fs::metadata(path).map_err(|e| fail(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(fail(format!(
            "expected a regular{encoding} file of at most {limit}"
        )));
    }
    let read_limit = max_bytes
        .checked_add(1)
        .ok_or_else(|| fail("invalid input-size limit".into()))?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| fail(e.to_string()))?
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|e| fail(e.to_string()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(fail(format!("{noun} exceeds {limit}")));
    }
    let digest = sha256(&bytes);
    Ok((
        bytes,
        SourceAnchor::new(
            name,
            digest.0,
            model_locator.map_or_else(|| format!("declared {kind} source"), str::to_string),
        ),
    ))
}

pub(super) fn source_json(source: &SourceAnchor, bytes: usize) -> String {
    format!(
        "{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{bytes}}}",
        crate::escape_json(&source.artifact_path),
        Digest32(source.artifact_sha256)
    )
}
