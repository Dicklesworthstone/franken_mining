//! The `FMT-ANCHOR-1` ledger anchor (`fmine_core::anchor::LedgerAnchor`, BD-017).
//!
//! Input: a mode byte, then the frame (see `fmine_fuzz::shape`; the wrapped frame's version is 0, 1 or the
//! unsupported 2). An accepted frame must be the canonical encoding of the anchor it decodes to: otherwise two byte
//! strings would name one anchor, and its id, which hashes the re-encoding, would not hash the stored bytes.
#![no_main]

use fmine_core::anchor::LedgerAnchor;
use fmine_fuzz::shape;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let bytes = shape(mode, *b"ANCR", u16::from(mode / 3 % 3), rest);
    if let Ok(anchor) = LedgerAnchor::from_bytes(&bytes) {
        assert_eq!(
            anchor.to_bytes(),
            bytes,
            "accepted a non-canonical anchor frame"
        );
        let _ = anchor.anchor_id();
        let _ = anchor.digest();
    }
});
