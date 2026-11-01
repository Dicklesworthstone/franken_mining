//! The durable journals: forecast-operation records (`fmine_claim::forecast_journal`, frames `FJR1`) and reliance
//! records (`fmine_claim::reliance`, frames `RLYJ`).
//!
//! Input: a selector byte, then the bytes. `selector % 4` picks the decoder: one forecast frame, one forecast hex line,
//! a whole forecast journal, or one reliance frame. For frames, `selector / 4` picks the shape (`fmine_fuzz::shape`).
//! Whatever a decoder accepts must re-encode to exactly the bytes it read.
#![no_main]

use fmine_claim::forecast_journal::{Record, decode_journal};
use fmine_claim::reliance::JournalRecord;
use fmine_fuzz::shape;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let mode = selector / 4;
    match selector % 4 {
        0 => {
            let bytes = shape(mode, *b"FJR1", 1, rest);
            if let Ok(record) = Record::from_bytes(&bytes) {
                assert_eq!(
                    record.to_bytes().ok(),
                    Some(bytes),
                    "forecast frame is not canonical"
                );
                let line = record
                    .to_line()
                    .expect("an accepted record encodes as a line");
                assert_eq!(
                    Record::from_line(&line).ok(),
                    Some(record),
                    "line round trip changed the record"
                );
            }
        }
        1 => {
            if let Ok(record) = Record::from_line(rest) {
                assert_eq!(
                    record.to_line().ok().as_deref(),
                    Some(rest),
                    "forecast line is not canonical"
                );
            }
        }
        2 => {
            if let Ok(records) = decode_journal(rest) {
                let mut again = Vec::with_capacity(rest.len());
                for record in &records {
                    again.extend(
                        record
                            .to_line()
                            .expect("an accepted record encodes as a line"),
                    );
                }
                assert_eq!(again, rest, "journal re-encodes to different bytes");
            }
        }
        _ => {
            let bytes = shape(mode, *b"RLYJ", 1, rest);
            if let Ok(record) = JournalRecord::from_bytes(&bytes) {
                assert_eq!(
                    record.to_bytes().ok(),
                    Some(bytes),
                    "reliance frame is not canonical"
                );
            }
        }
    }
});
