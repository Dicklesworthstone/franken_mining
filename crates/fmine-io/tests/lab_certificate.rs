//! Lab certificate originals in CAS (`BD-171`'s done-when: digest-linked). An ingested certificate's record
//! names its copy by the original's SHA-256 and by the copy's `FMT-CAS-1` root; the record survives the drill
//! codec, and reading its root back gives the original. A copy that was swapped, damaged, never stored or stored
//! under another media type does not verify, and an original that does not match the digest the lab stated is
//! refused before anything is stored.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_core::hash::{Digest32, sha256};
use fmine_drill::records::{DrillRecord, LabCertificate, decode_version, encode_version};
use fmine_evidence::RefusalCode;
use fmine_io::lab_certificate::{
    CertificateError, CertificateOriginal, ingest_certificate, verify_certificate,
};
use fmine_object::cas::{CasError, DEFAULT_CHUNK_BYTES};
use fmine_object::store::CasStore;

const ISSUED: i64 = 1_790_600_000_000_000_000;

fn store() -> CasStore {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "fmine-io-lab-certificate-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    CasStore::open(dir).unwrap()
}

/// Deterministic bytes standing in for a signed PDF.
fn pdf(seed: u8, len: usize) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut x = u32::from(seed) | 1;
    while out.len() < len {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        out.push(x.to_le_bytes()[0]);
    }
    out.truncate(len);
    out
}

fn original<'a>(certificate_id: &'a str, bytes: &'a [u8]) -> CertificateOriginal<'a> {
    CertificateOriginal {
        certificate_id,
        lab: "LAB-A",
        media_type: "application/pdf",
        issued_at_ns: ISSUED,
        bytes,
        stated_sha256: None,
    }
}

fn code(e: &CertificateError) -> &'static str {
    e.to_refusal().code
}

#[test]
fn test_an_ingested_certificate_is_linked_to_its_copy_by_digest_and_root() {
    let cas = store();
    // One chunk, and three (two full and a short last one).
    for (n, len) in [(1_u8, 48_211_usize), (2, 2 * DEFAULT_CHUNK_BYTES + 1_000)] {
        let bytes = pdf(n, len);
        let stated = sha256(&bytes);
        let id = format!("CERT-{n}");
        let got = ingest_certificate(
            &cas,
            &CertificateOriginal {
                stated_sha256: Some(stated),
                ..original(&id, &bytes)
            },
        )
        .unwrap();
        assert_eq!(got.record.original_sha256, stated);
        assert_eq!(got.total_bytes, len as u64);
        assert_eq!(got.chunk_count, len.div_ceil(DEFAULT_CHUNK_BYTES));
        assert_eq!(
            cas.get_manifest(&got.record.cas_root).unwrap().root_digest,
            got.record.cas_root
        );

        // The record goes through the drill codec unchanged and still verifies against the store.
        let frame = encode_version(&DrillRecord::LabCertificate(got.record.clone()), None).unwrap();
        let DrillRecord::LabCertificate(decoded) = decode_version(&frame).unwrap().record else {
            panic!("not a certificate");
        };
        assert_eq!(decoded, got.record);
        assert_eq!(verify_certificate(&cas, &decoded).unwrap(), len as u64);
        assert_eq!(cas.read_object(&decoded.cas_root).unwrap(), bytes);
    }
    assert_eq!(cas.list_published_roots().unwrap().len(), 2);
}

#[test]
fn test_the_same_original_again_gives_the_same_root_and_writes_no_chunk() {
    let cas = store();
    let bytes = pdf(7, DEFAULT_CHUNK_BYTES + 77);
    let first = ingest_certificate(&cas, &original("CERT-7", &bytes)).unwrap();
    let chunks = cas.list_chunks().unwrap();
    assert_eq!(chunks.len(), 2);
    // Issued again under another id, or received a second time: one copy.
    let again = ingest_certificate(&cas, &original("CERT-7-REISSUE", &bytes)).unwrap();
    assert_eq!(again.record.cas_root, first.record.cas_root);
    assert_eq!(again.record.original_sha256, first.record.original_sha256);
    assert_eq!(cas.list_chunks().unwrap(), chunks);
    assert_eq!(
        cas.list_published_roots().unwrap(),
        vec![first.record.cas_root]
    );
}

#[test]
fn test_an_original_that_cannot_be_linked_is_refused_and_nothing_is_stored() {
    let cas = store();
    let bytes = pdf(3, 10_000);
    let mut wrong = sha256(&bytes);
    wrong.0[0] ^= 1;
    let err = ingest_certificate(
        &cas,
        &CertificateOriginal {
            stated_sha256: Some(wrong),
            ..original("CERT-3", &bytes)
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, CertificateError::DigestMismatch { .. }),
        "{err}"
    );
    assert_eq!(code(&err), RefusalCode::DigestMismatch505.spec().code);

    let err = ingest_certificate(&cas, &original("CERT-0", &[])).unwrap_err();
    assert!(matches!(err, CertificateError::Empty { .. }), "{err}");
    assert_eq!(
        code(&err),
        RefusalCode::InputMissingOrUnreadable506.spec().code
    );

    for bad in [
        CertificateOriginal {
            media_type: "",
            ..original("CERT-3", &bytes)
        },
        original("CERT/3", &bytes),
    ] {
        let err = ingest_certificate(&cas, &bad).unwrap_err();
        assert!(matches!(err, CertificateError::Record(_)), "{err}");
        assert_eq!(
            code(&err),
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }
    assert!(cas.list_published_roots().unwrap().is_empty());
    assert!(cas.list_chunks().unwrap().is_empty());
}

#[test]
fn test_a_swapped_damaged_missing_or_retyped_copy_does_not_verify() {
    let cas = store();
    let a = pdf(11, 30_000);
    let b = pdf(12, 30_000);
    let rec_a = ingest_certificate(&cas, &original("CERT-A", &a))
        .unwrap()
        .record;
    let rec_b = ingest_certificate(&cas, &original("CERT-B", &b))
        .unwrap()
        .record;
    verify_certificate(&cas, &rec_a).unwrap();

    // Swapped: A's record pointed at B's copy.
    let swapped = LabCertificate {
        cas_root: rec_b.cas_root,
        ..rec_a.clone()
    };
    let err = verify_certificate(&cas, &swapped).unwrap_err();
    assert!(
        matches!(err, CertificateError::DigestMismatch { expected, actual, .. }
            if expected == rec_a.original_sha256 && actual == rec_b.original_sha256),
        "{err}"
    );
    assert_eq!(code(&err), RefusalCode::DigestMismatch505.spec().code);

    // Never stored.
    let missing = LabCertificate {
        cas_root: Digest32([0x5a; 32]),
        ..rec_a.clone()
    };
    let err = verify_certificate(&cas, &missing).unwrap_err();
    assert!(
        matches!(
            err,
            CertificateError::Cas(CasError::ManifestNotFound { .. })
        ),
        "{err}"
    );

    // Stored as a PDF, recorded as something else.
    let retyped = LabCertificate {
        media_type: "image/tiff".to_string(),
        ..rec_a.clone()
    };
    let err = verify_certificate(&cas, &retyped).unwrap_err();
    assert!(
        matches!(err, CertificateError::MediaTypeMismatch { .. }),
        "{err}"
    );
    assert_eq!(code(&err), RefusalCode::DigestMismatch505.spec().code);

    // Damaged on disk: one byte of B's only chunk flipped.
    let manifest = cas.get_manifest(&rec_b.cas_root).unwrap();
    let path = cas.chunk_path(&manifest.chunks[0].sha256);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[100] ^= 0xff;
    std::fs::write(&path, &bytes).unwrap();
    let err = verify_certificate(&cas, &rec_b).unwrap_err();
    assert!(
        matches!(
            err,
            CertificateError::Cas(CasError::ChunkDigestMismatch { .. })
        ),
        "{err}"
    );
    verify_certificate(&cas, &rec_a).unwrap();
}
