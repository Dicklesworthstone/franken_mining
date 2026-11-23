//! CLI rendering for the common bounded source reader.

use fmine_core::{Digest32, anchor::SourceAnchor};

pub(super) use fmine_io::source::{read, read_bytes, read_model};

pub(super) fn markdown_cell(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\\' | '|' | '`' | '[' | ']' | '*' | '_' | '~' => {
                escaped.push('\\');
                escaped.push(c);
            }
            c if c.is_control() => escaped.push(' '),
            c => escaped.push(c),
        }
    }
    escaped
}

pub(super) fn source_json(source: &SourceAnchor, bytes: usize) -> String {
    format!(
        "{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{bytes}}}",
        crate::escape_json(&source.artifact_path),
        Digest32(source.artifact_sha256)
    )
}
