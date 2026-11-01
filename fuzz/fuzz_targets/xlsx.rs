//! The workbook reader (`fmine_io`: the bounded zip reader, the inflater, the XML parts, styles and number formats).
//!
//! Input: the bytes of a would-be `.xlsx`. No entry may inflate past `MAX_ENTRY_BYTES` and no archive past
//! `MAX_ARCHIVE_BYTES`; an accepted workbook must survive every census and every displayed value.
#![no_main]

use fmine_io::xlsx::Workbook;
use fmine_io::zip_inflate::{MAX_ARCHIVE_BYTES, MAX_ENTRY_BYTES, read_zip_entries};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(entries) = read_zip_entries(data) {
        let mut total = 0usize;
        for (name, bytes) in &entries {
            assert!(
                bytes.len() <= MAX_ENTRY_BYTES,
                "entry {name} holds {} bytes",
                bytes.len()
            );
            total += bytes.len();
        }
        assert!(total <= MAX_ARCHIVE_BYTES, "archive holds {total} bytes");
    }
    if let Ok(wb) = Workbook::from_bytes("fuzz.xlsx", data) {
        let _ = wb.cell_census();
        let _ = wb.format_census();
        let _ = wb.displayed_census();
        let _ = wb.total_cells();
        for sheet in wb.sheets.iter().chain(&wb.hidden_sheets) {
            for cell in sheet.cells_by_coord.values() {
                let _ = cell.value.as_text();
                if let Some(Ok(d)) = cell.displayed() {
                    let _ = d.text();
                }
            }
        }
    }
});
