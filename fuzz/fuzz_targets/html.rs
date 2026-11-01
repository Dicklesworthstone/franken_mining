//! HTML table extraction (`fmine_io::extract_html_tables`).
//!
//! Input: UTF-8 text. Every extracted table must be walkable.
#![no_main]

use fmine_io::extract_html_tables;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(html) = std::str::from_utf8(data) else {
        return;
    };
    for table in extract_html_tables(html) {
        let _ = format!("{table:?}");
    }
});
