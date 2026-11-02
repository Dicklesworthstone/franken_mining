//! Bounded worksheet reads retain the exact submitted bytes and their source identity.

use std::io::Read as _;
use std::path::Path;

use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};

pub(super) fn read(
    path: &Path,
    kind: &str,
    max_bytes: u64,
) -> Result<(String, SourceAnchor), Refusal> {
    let fail = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{kind} input {}: {why}", path.display()),
        )
    };
    let limit = if max_bytes.is_multiple_of(1024 * 1024) {
        format!("{} MiB", max_bytes / (1024 * 1024))
    } else {
        format!("{max_bytes} bytes")
    };
    let name = path
        .to_str()
        .ok_or_else(|| fail("path must be UTF-8".into()))?;
    let metadata = std::fs::metadata(path).map_err(|e| fail(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(fail(format!(
            "expected a regular UTF-8 file of at most {limit}"
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
        return Err(fail(format!("input exceeds {limit}")));
    }
    let digest = sha256(&bytes);
    let text = String::from_utf8(bytes).map_err(|e| fail(e.to_string()))?;
    Ok((
        text,
        SourceAnchor::new(name, digest.0, format!("declared {kind} source")),
    ))
}
