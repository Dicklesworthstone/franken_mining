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
}
