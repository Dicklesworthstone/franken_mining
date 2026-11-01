//! Canonical binary codec kit (`FMNE` magic, LE, bounds, CRC32C, canonical order; BD-017).
//! Determinism: BitwisePortable

use core::fmt;

/// Universal 4-byte magic prefix for FrankenMining binary formats (`§26.5`, `§28.1`).
pub const MAGIC_FMNE: [u8; 4] = *b"FMNE";

/// Maximum permitted slice/frame length in canonical decoding (64 MiB).
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Errors returned by [`Encoder`] / [`Decoder`] (`BD-017`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    /// Frame did not begin with `FMNE`.
    InvalidMagic([u8; 4]),
    /// Frame tag did not match expected format tag.
    UnexpectedTag {
        /// Tag found in header.
        actual: [u8; 4],
        /// Tag expected by decoder.
        expected: [u8; 4],
    },
    /// Schema version is not supported by this reader (supports N and N-1).
    UnsupportedVersion {
        /// Version found in header.
        actual: u16,
        /// Minimum supported version.
        min_supported: u16,
        /// Maximum supported version.
        max_supported: u16,
    },
    /// Unexpected end of buffer.
    UnexpectedEof {
        /// Bytes needed.
        needed: usize,
        /// Bytes remaining.
        remaining: usize,
    },
    /// Declared length exceeds [`MAX_FRAME_BYTES`] or caller bound.
    LengthOutOfBounds {
        /// Declared length.
        len: usize,
        /// Maximum permitted length.
        max: usize,
    },
    /// Invalid UTF-8 string in binary payload.
    InvalidUtf8,
    /// Non-canonical key ordering in serialized map or set.
    NonCanonicalOrder,
    /// Trailing unconsumed bytes after frame payload + CRC32C.
    TrailingBytes(usize),
    /// Castagnoli CRC32C checksum mismatch.
    CrcMismatch {
        /// Checksum computed over frame bytes.
        computed: u32,
        /// Checksum stored in frame footer.
        stored: u32,
    },
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic(m) => write!(f, "invalid FMNE magic: {m:?}"),
            Self::UnexpectedTag { actual, expected } => {
                write!(f, "unexpected format tag {actual:?}, expected {expected:?}")
            }
            Self::UnsupportedVersion {
                actual,
                min_supported,
                max_supported,
            } => write!(
                f,
                "unsupported format version {actual} (supported {min_supported}..={max_supported})"
            ),
            Self::UnexpectedEof { needed, remaining } => {
                write!(f, "unexpected EOF: needed {needed} bytes, {remaining} left")
            }
            Self::LengthOutOfBounds { len, max } => {
                write!(f, "length {len} exceeds bound {max}")
            }
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 string in binary frame"),
            Self::NonCanonicalOrder => write!(f, "elements are not in strict canonical order"),
            Self::TrailingBytes(n) => write!(f, "{n} trailing bytes after frame"),
            Self::CrcMismatch { computed, stored } => {
                write!(
                    f,
                    "CRC32C mismatch: computed {computed:#010x} != stored {stored:#010x}"
                )
            }
        }
    }
}

impl std::error::Error for CodecError {}

/// Compute Castagnoli CRC32C (polynomial `0x1EDC6F41`, reflected `0x82F63B78`) (`BD-017`).
#[must_use]
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0x82F6_3B78 & mask);
        }
    }
    !crc
}

/// Canonical little-endian frame encoder (`BD-017`).
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    /// Begin a new canonical `FMNE` frame with a 4-byte format tag and 16-bit schema version.
    #[must_use]
    pub fn new_frame(tag: [u8; 4], version: u16) -> Self {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&MAGIC_FMNE);
        buf.extend_from_slice(&tag);
        buf.extend_from_slice(&version.to_le_bytes());
        Self { buf }
    }

    /// Write a single `u8`.
    pub fn put_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Write a little-endian `u16`.
    pub fn put_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a little-endian `u32`.
    pub fn put_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a little-endian `u64`.
    pub fn put_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a little-endian `i64`.
    pub fn put_i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a little-endian `i128`.
    pub fn put_i128(&mut self, v: i128) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write fixed-length bytes.
    pub fn put_fixed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Write length-prefixed byte slice (`u32` LE length + bytes).
    pub fn put_bytes(&mut self, bytes: &[u8]) {
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        self.put_u32(len);
        self.buf.extend_from_slice(bytes);
    }

    /// Write length-prefixed UTF-8 string.
    pub fn put_str(&mut self, s: &str) {
        self.put_bytes(s.as_bytes());
    }

    /// Write a sequence in canonical order: a `u32` count, then each element by `write_one`. The elements must
    /// already be strictly ascending, as a `BTreeSet` or a `BTreeMap`'s keys iterate, so that a set or a map has
    /// exactly one encoding. Out-of-order or repeated elements are refused with [`CodecError::NonCanonicalOrder`],
    /// and more than `u32::MAX` elements with [`CodecError::LengthOutOfBounds`]; nothing is written then.
    pub fn put_canonical_seq<T: Ord>(
        &mut self,
        items: &[T],
        mut write_one: impl FnMut(&mut Self, &T),
    ) -> Result<(), CodecError> {
        if items.windows(2).any(|w| w[0] >= w[1]) {
            return Err(CodecError::NonCanonicalOrder);
        }
        let n = u32::try_from(items.len()).map_err(|_| CodecError::LengthOutOfBounds {
            len: items.len(),
            max: u32::MAX as usize,
        })?;
        self.put_u32(n);
        for item in items {
            write_one(self, item);
        }
        Ok(())
    }

    /// Finalize the frame by appending its 4-byte little-endian CRC32C checksum.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        let crc = crc32c(&self.buf);
        self.buf.extend_from_slice(&crc.to_le_bytes());
        self.buf
    }
}

/// Canonical little-endian frame decoder (`BD-017`).
#[derive(Clone, Debug)]
pub struct Decoder<'a> {
    buf: &'a [u8],
    pos: usize,
    version: u16,
}

impl<'a> Decoder<'a> {
    /// Open and verify an `FMNE` frame's magic, format tag, version range `[min_ver, max_ver]`, and trailing CRC32C.
    pub fn open_frame(
        bytes: &'a [u8],
        expected_tag: [u8; 4],
        min_supported: u16,
        max_supported: u16,
    ) -> Result<Self, CodecError> {
        if bytes.len() < 14 {
            return Err(CodecError::UnexpectedEof {
                needed: 14,
                remaining: bytes.len(),
            });
        }
        let payload_end = bytes.len() - 4;
        let stored_crc = u32::from_le_bytes([
            bytes[payload_end],
            bytes[payload_end + 1],
            bytes[payload_end + 2],
            bytes[payload_end + 3],
        ]);
        let computed_crc = crc32c(&bytes[..payload_end]);
        if computed_crc != stored_crc {
            return Err(CodecError::CrcMismatch {
                computed: computed_crc,
                stored: stored_crc,
            });
        }
        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if magic != MAGIC_FMNE {
            return Err(CodecError::InvalidMagic(magic));
        }
        let tag = [bytes[4], bytes[5], bytes[6], bytes[7]];
        if tag != expected_tag {
            return Err(CodecError::UnexpectedTag {
                actual: tag,
                expected: expected_tag,
            });
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version < min_supported || version > max_supported {
            return Err(CodecError::UnsupportedVersion {
                actual: version,
                min_supported,
                max_supported,
            });
        }
        Ok(Self {
            buf: &bytes[..payload_end],
            pos: 10,
            version,
        })
    }

    /// Schema version decoded from the frame header.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Read a Fixed slice of `N` bytes.
    pub fn get_fixed<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let rem = self.buf.len().saturating_sub(self.pos);
        if rem < N {
            return Err(CodecError::UnexpectedEof {
                needed: N,
                remaining: rem,
            });
        }
        let mut out = [0u8; N];
        out.copy_from_slice(&self.buf[self.pos..self.pos + N]);
        self.pos += N;
        Ok(out)
    }

    /// Read a `u8`.
    pub fn get_u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.get_fixed::<1>()?[0])
    }

    /// Read a little-endian `u16`.
    pub fn get_u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.get_fixed::<2>()?))
    }

    /// Read a little-endian `u32`.
    pub fn get_u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.get_fixed::<4>()?))
    }

    /// Read a little-endian `u64`.
    pub fn get_u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.get_fixed::<8>()?))
    }

    /// Read a little-endian `i64`.
    pub fn get_i64(&mut self) -> Result<i64, CodecError> {
        Ok(i64::from_le_bytes(self.get_fixed::<8>()?))
    }

    /// Read a little-endian `i128`.
    pub fn get_i128(&mut self) -> Result<i128, CodecError> {
        Ok(i128::from_le_bytes(self.get_fixed::<16>()?))
    }

    /// Read length-prefixed bytes with an upper bound `max_len`.
    pub fn get_bytes(&mut self, max_len: usize) -> Result<&'a [u8], CodecError> {
        let len = self.get_u32()? as usize;
        let bound = max_len.min(MAX_FRAME_BYTES);
        if len > bound {
            return Err(CodecError::LengthOutOfBounds { len, max: bound });
        }
        let rem = self.buf.len().saturating_sub(self.pos);
        if rem < len {
            return Err(CodecError::UnexpectedEof {
                needed: len,
                remaining: rem,
            });
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    /// Read a length-prefixed UTF-8 string with an upper bound `max_len`.
    pub fn get_str(&mut self, max_len: usize) -> Result<&'a str, CodecError> {
        let bytes = self.get_bytes(max_len)?;
        core::str::from_utf8(bytes).map_err(|_| CodecError::InvalidUtf8)
    }

    /// Read a sequence written by [`Encoder::put_canonical_seq`]: a `u32` count of at most `max_items`, then that
    /// many elements read by `read_one`, each strictly greater than the one before. A repeated or out-of-order
    /// element is [`CodecError::NonCanonicalOrder`]. Memory is reserved for no more elements than there are bytes
    /// left, so a large count in a short frame cannot force a large allocation.
    pub fn get_canonical_seq<T: Ord>(
        &mut self,
        max_items: usize,
        mut read_one: impl FnMut(&mut Self) -> Result<T, CodecError>,
    ) -> Result<Vec<T>, CodecError> {
        let n = self.get_u32()? as usize;
        if n > max_items {
            return Err(CodecError::LengthOutOfBounds {
                len: n,
                max: max_items,
            });
        }
        let mut out: Vec<T> = Vec::with_capacity(n.min(self.buf.len().saturating_sub(self.pos)));
        for _ in 0..n {
            let item = read_one(self)?;
            if out.last().is_some_and(|prev| *prev >= item) {
                return Err(CodecError::NonCanonicalOrder);
            }
            out.push(item);
        }
        Ok(out)
    }

    /// Assert that all payload bytes before the CRC32C footer were consumed.
    pub fn finish(&self) -> Result<(), CodecError> {
        let rem = self.buf.len().saturating_sub(self.pos);
        if rem != 0 {
            Err(CodecError::TrailingBytes(rem))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32c_iscsi_known_answer() {
        // RFC 3720 / iSCSI standard check value for "123456789" is 0xE3069283.
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    }

    #[test]
    fn test_frame_roundtrip_and_tamper_detection() {
        let mut enc = Encoder::new_frame(*b"TEST", 1);
        enc.put_u64(0xCAFE_BABE_1234_5678);
        enc.put_str("ngm-carlin");
        let mut bytes = enc.finish();

        let mut dec = Decoder::open_frame(&bytes, *b"TEST", 1, 1).unwrap();
        assert_eq!(dec.get_u64().unwrap(), 0xCAFE_BABE_1234_5678);
        assert_eq!(dec.get_str(64).unwrap(), "ngm-carlin");
        dec.finish().unwrap();

        // Flip 1 bit in payload -> CRC32C must fail.
        bytes[12] ^= 0x01;
        assert!(matches!(
            Decoder::open_frame(&bytes, *b"TEST", 1, 1),
            Err(CodecError::CrcMismatch { .. })
        ));
    }

    /// A frame with `tag`, `version` and the body `write` produces.
    fn framed(tag: [u8; 4], version: u16, write: impl FnOnce(&mut Encoder)) -> Vec<u8> {
        let mut enc = Encoder::new_frame(tag, version);
        write(&mut enc);
        enc.finish()
    }

    /// `bytes` with the frame's leading bytes replaced and its CRC32C recomputed, so only the header is wrong.
    fn reheader(mut bytes: Vec<u8>, at: usize, with: &[u8]) -> Vec<u8> {
        bytes[at..at + with.len()].copy_from_slice(with);
        let end = bytes.len() - 4;
        let crc = crc32c(&bytes[..end]).to_le_bytes();
        bytes[end..].copy_from_slice(&crc);
        bytes
    }

    #[test]
    fn test_each_header_error_is_its_own_variant() {
        let good = framed(*b"TEST", 2, |e| e.put_u8(7));
        assert_eq!(
            Decoder::open_frame(&good[..13], *b"TEST", 1, 2).unwrap_err(),
            CodecError::UnexpectedEof {
                needed: 14,
                remaining: 13
            }
        );
        assert_eq!(
            Decoder::open_frame(&reheader(good.clone(), 0, b"FMNX"), *b"TEST", 1, 2).unwrap_err(),
            CodecError::InvalidMagic(*b"FMNX")
        );
        assert_eq!(
            Decoder::open_frame(&good, *b"OTHR", 1, 2).unwrap_err(),
            CodecError::UnexpectedTag {
                actual: *b"TEST",
                expected: *b"OTHR"
            }
        );
        for (min, max) in [(3, 4), (0, 1)] {
            assert_eq!(
                Decoder::open_frame(&good, *b"TEST", min, max).unwrap_err(),
                CodecError::UnsupportedVersion {
                    actual: 2,
                    min_supported: min,
                    max_supported: max
                }
            );
        }
        // Readers accept N and N-1.
        for version in [1, 2] {
            let bytes = framed(*b"TEST", version, |e| e.put_u8(7));
            assert_eq!(
                Decoder::open_frame(&bytes, *b"TEST", 1, 2)
                    .unwrap()
                    .version(),
                version
            );
        }
    }

    #[test]
    fn test_each_body_error_is_its_own_variant() {
        let bytes = framed(*b"TEST", 1, |e| {
            e.put_u16(9);
            e.put_bytes(&[0xFF, 0xFE]);
        });
        let mut dec = Decoder::open_frame(&bytes, *b"TEST", 1, 1).unwrap();
        assert_eq!(dec.get_u16().unwrap(), 9);
        let mut short = dec.clone();
        assert_eq!(
            short.get_bytes(1).unwrap_err(),
            CodecError::LengthOutOfBounds { len: 2, max: 1 }
        );
        assert_eq!(dec.clone().get_str(8).unwrap_err(), CodecError::InvalidUtf8);
        assert_eq!(
            dec.clone().finish().unwrap_err(),
            CodecError::TrailingBytes(6)
        );
        assert_eq!(
            dec.get_i128().unwrap_err(),
            CodecError::UnexpectedEof {
                needed: 16,
                remaining: 6
            }
        );

        // A length prefix past the bytes left is an EOF, not a read past the frame.
        let long = framed(*b"TEST", 1, |e| e.put_u32(1_000));
        let mut dec = Decoder::open_frame(&long, *b"TEST", 1, 1).unwrap();
        assert_eq!(
            dec.get_bytes(usize::MAX).unwrap_err(),
            CodecError::UnexpectedEof {
                needed: 1_000,
                remaining: 0
            }
        );
        // The global cap applies whatever the caller's bound.
        let huge = framed(*b"TEST", 1, |e| e.put_u32(u32::MAX));
        let mut dec = Decoder::open_frame(&huge, *b"TEST", 1, 1).unwrap();
        assert_eq!(
            dec.get_bytes(usize::MAX).unwrap_err(),
            CodecError::LengthOutOfBounds {
                len: u32::MAX as usize,
                max: MAX_FRAME_BYTES
            }
        );
    }

    #[test]
    fn test_canonical_seq_round_trips_and_refuses_disorder() {
        let set: std::collections::BTreeSet<u64> = [40, 3, 17, 99].into_iter().collect();
        let items: Vec<u64> = set.iter().copied().collect();
        let bytes = framed(*b"TEST", 1, |e| {
            e.put_canonical_seq(&items, |e, v| e.put_u64(*v)).unwrap();
        });
        let mut dec = Decoder::open_frame(&bytes, *b"TEST", 1, 1).unwrap();
        assert_eq!(dec.get_canonical_seq(4, Decoder::get_u64).unwrap(), items);
        dec.finish().unwrap();

        // The encoder refuses disorder and repeats, and writes nothing.
        for bad in [vec![3u64, 2], vec![5, 5]] {
            let mut enc = Encoder::new_frame(*b"TEST", 1);
            assert_eq!(
                enc.put_canonical_seq(&bad, |e, v| e.put_u64(*v)),
                Err(CodecError::NonCanonicalOrder)
            );
            assert_eq!(enc.finish().len(), 14);
        }
        // The decoder refuses them too, and a count over the caller's bound.
        for (count, body) in [(2u32, [3u64, 2]), (2, [5, 5])] {
            let bytes = framed(*b"TEST", 1, |e| {
                e.put_u32(count);
                body.iter().for_each(|v| e.put_u64(*v));
            });
            let mut dec = Decoder::open_frame(&bytes, *b"TEST", 1, 1).unwrap();
            assert_eq!(
                dec.clone()
                    .get_canonical_seq(2, Decoder::get_u64)
                    .unwrap_err(),
                CodecError::NonCanonicalOrder
            );
            assert_eq!(
                dec.get_canonical_seq(1, Decoder::get_u64).unwrap_err(),
                CodecError::LengthOutOfBounds { len: 2, max: 1 }
            );
        }
        // A huge count in a short frame fails at the first missing element, without reserving for the count.
        let bytes = framed(*b"TEST", 1, |e| e.put_u32(u32::MAX));
        let mut dec = Decoder::open_frame(&bytes, *b"TEST", 1, 1).unwrap();
        assert!(matches!(
            dec.get_canonical_seq(usize::MAX, Decoder::get_u64),
            Err(CodecError::UnexpectedEof { needed: 8, .. })
        ));
    }

    #[test]
    fn test_every_error_displays_a_message() {
        let all = [
            CodecError::InvalidMagic(*b"FMNX"),
            CodecError::UnexpectedTag {
                actual: *b"AAAA",
                expected: *b"BBBB",
            },
            CodecError::UnsupportedVersion {
                actual: 9,
                min_supported: 1,
                max_supported: 2,
            },
            CodecError::UnexpectedEof {
                needed: 2,
                remaining: 1,
            },
            CodecError::LengthOutOfBounds { len: 2, max: 1 },
            CodecError::InvalidUtf8,
            CodecError::NonCanonicalOrder,
            CodecError::TrailingBytes(3),
            CodecError::CrcMismatch {
                computed: 1,
                stored: 2,
            },
        ];
        let texts: std::collections::BTreeSet<String> =
            all.iter().map(ToString::to_string).collect();
        assert_eq!(texts.len(), all.len(), "two errors print the same text");
    }
}
