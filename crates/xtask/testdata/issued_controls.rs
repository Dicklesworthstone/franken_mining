use super::*;

#[test]
fn issuance_requires_a_complete_build_revision_without_inventing_metadata() {
    let clean = "1".repeat(40);
    assert_eq!(checked_revision(&clean).expect("revision"), clean);
    let dirty = format!("{clean}+dirty");
    assert_eq!(checked_revision(&dirty).expect("dirty revision"), dirty);
    for invalid in [
        "unknown",
        "",
        "1234567",
        "unversioned+dirty",
        "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
    ] {
        assert_eq!(
            checked_revision(invalid)
                .expect_err("unknown provenance")
                .code,
            "FMINE-REF-202"
        );
    }
}

#[cfg(target_os = "linux")]
mod image_identity {
    use super::*;
    use std::io::{self, Cursor, Read};

    struct Chunked<R> {
        inner: R,
        interrupt_next: bool,
        total: u64,
        largest_request: usize,
    }

    impl<R: Read> Read for Chunked<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.largest_request = self.largest_request.max(buffer.len());
            if self.interrupt_next {
                self.interrupt_next = false;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = buffer.len().min(4093);
            let read = self.inner.read(&mut buffer[..count])?;
            self.total += read as u64;
            Ok(read)
        }
    }

    fn chunked<R: Read>(inner: R) -> Chunked<R> {
        Chunked {
            inner,
            interrupt_next: true,
            total: 0,
            largest_request: 0,
        }
    }

    #[test]
    fn streaming_image_identity_matches_exact_bytes_across_read_boundaries() {
        for length in [
            1, 63, 64, 65, 1023, 1024, 1025, 16_383, 16_384, 16_385, 32_769,
        ] {
            let bytes: Vec<u8> = (0..length).map(|i| (i % 251) as u8).collect();
            let mut reader = chunked(Cursor::new(&bytes));
            let got = digest_image_reader(&mut reader, bytes.len() as u64)
                .expect("complete interrupted/short reads");
            assert_eq!(got, blake3_hash(&bytes), "length {length}");
            assert_eq!(reader.total, length as u64);
            assert!(reader.largest_request <= 16 * 1024);
        }
    }

    #[test]
    fn executable_identity_is_not_limited_by_report_artifact_size() {
        let length = fmine_io::reliance::MAX_ARTIFACT_BYTES + 1;
        let mut reader = chunked(io::repeat(0xa5).take(length));
        digest_image_reader(&mut reader, length)
            .expect("running images may exceed the independent report-artifact limit");
        assert_eq!(reader.total, length);
        assert!(reader.largest_request <= 16 * 1024);
    }

    #[test]
    fn image_identity_refuses_length_changes_and_read_failures() {
        for expected in [0, 2, 4, u64::MAX] {
            let mut reader = Cursor::new(b"abc");
            let error = digest_image_reader(&mut reader, expected)
                .expect_err("length must be exactly supported");
            assert_eq!(error.code, "FMINE-REF-506");
            assert!(reader.position() <= expected.saturating_add(1));
        }

        struct ReadFailure;
        impl Read for ReadFailure {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected image read failure"))
            }
        }
        // A failure after all declared bytes still must not be accepted as end-of-file.
        let error = digest_image_reader(Cursor::new(b"abc").chain(ReadFailure), 3)
            .expect_err("read errors do not prove EOF");
        assert_eq!(error.code, "FMINE-REF-506");
        assert!(error.what.contains("injected image read failure"));
    }
}
