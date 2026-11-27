//! The `FMT-CAS-1` manifest decoder (`fmine_object::cas::CasManifest::from_bytes`; BD-090, BD-091).
//!
//! A manifest reaches this decoder from a peer during sync and from a staging marker on disk. Input: a mode byte,
//! then the frame (see `fmine_fuzz::shape`; wrapped versions 0, 1, or unsupported 2). Any accepted manifest must
//! re-encode byte for byte to the input. Its fanout-16 inclusion proof must verify for each of the first 64
//! chunks, and must fail for that chunk's digest with one bit flipped.
#![no_main]

use fmine_fuzz::shape;
use fmine_object::cas::CasManifest;
use fmine_object::merkle::{inclusion_proof, verify_inclusion};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let bytes = shape(mode, *b"CASM", u16::from(mode / 3 % 3), rest);
    if let Ok(manifest) = CasManifest::from_bytes(&bytes) {
        let reencoded = manifest
            .to_bytes()
            .expect("decoded CasManifest must re-encode cleanly");
        assert_eq!(
            reencoded, bytes,
            "accepted a non-canonical FMT-CAS-1 manifest"
        );
        let digests: Vec<_> = manifest.chunks.iter().map(|c| c.sha256).collect();
        for (i, digest) in digests.iter().enumerate().take(64) {
            let proof = inclusion_proof(&digests, i).expect("a chunk in range has a proof");
            assert!(
                verify_inclusion(digest, &manifest.root_digest, &proof),
                "chunk {i}'s proof fails against the root it was accepted under"
            );
            let mut flipped = *digest;
            flipped.0[0] ^= 1;
            assert!(
                !verify_inclusion(&flipped, &manifest.root_digest, &proof),
                "chunk {i}'s proof verified a digest with one bit flipped"
            );
        }
    }
});
