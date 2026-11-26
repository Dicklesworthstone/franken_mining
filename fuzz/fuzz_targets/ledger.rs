//! The `FMT-LEDGER-1` delta batch (`fmine_ledger::batch::DeltaBatch`, BD-081).
//!
//! Input: a mode byte, then the frame (see `fmine_fuzz::shape`; wrapped versions 0, 1, 2, or unsupported 3).
//! Any accepted frame must re-encode byte-for-byte equal to the input. The re-encoding always ends in the right
//! CRC32C, so a frame accepted with a wrong one, as after a flipped bit, fails that check.
#![no_main]

use fmine_fuzz::shape;
use fmine_ledger::batch::DeltaBatch;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let bytes = shape(mode, *b"LDGR", u16::from(mode / 3 % 4), rest);
    if let Ok(batch) = DeltaBatch::from_bytes(&bytes) {
        let reencoded = batch
            .to_bytes()
            .expect("decoded DeltaBatch must re-encode cleanly");
        assert_eq!(
            reencoded, bytes,
            "accepted a non-canonical FMT-LEDGER-1 frame"
        );
        let _ = batch.batch_digest();
    }
});
