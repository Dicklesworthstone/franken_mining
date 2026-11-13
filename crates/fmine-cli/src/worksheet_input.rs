//! CLI rendering for the common bounded source reader.

use fmine_core::{Digest32, anchor::SourceAnchor};

pub(super) use fmine_io::source::{read, read_bytes, read_model};

pub(super) fn source_json(source: &SourceAnchor, bytes: usize) -> String {
    format!(
        "{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{bytes}}}",
        crate::escape_json(&source.artifact_path),
        Digest32(source.artifact_sha256)
    )
}
