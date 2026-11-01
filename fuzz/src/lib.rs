//! Helpers shared by the fuzz targets (plan §32.4, PO-PORT-1). Not part of the product.
//!
//! Every `FMNE` frame ends in a CRC32C over the rest, and the decoders check it first, so random bytes almost never
//! reach a frame body. The frame targets therefore read their input three ways, chosen by its first byte: as it is
//! (the checksum and header paths), with the checksum repaired (the header paths), and as a body wrapped in a valid
//! header and checksum (the body paths).

use fmine_core::codec::{MAGIC_FMNE, crc32c};

/// The smallest frame: magic, tag, version and checksum.
pub const MIN_FRAME: usize = 14;

/// `bytes` with its last four bytes replaced by the CRC32C of the rest. Shorter inputs are returned unchanged.
#[must_use]
pub fn with_crc(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    if out.len() >= MIN_FRAME {
        let end = out.len() - 4;
        let crc = crc32c(&out[..end]).to_le_bytes();
        out[end..].copy_from_slice(&crc);
    }
    out
}

/// A frame with header `FMNE`, `tag`, `version`, the given body and a correct CRC32C.
#[must_use]
pub fn frame(tag: [u8; 4], version: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + MIN_FRAME);
    out.extend_from_slice(&MAGIC_FMNE);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(body);
    let crc = crc32c(&out).to_le_bytes();
    out.extend_from_slice(&crc);
    out
}

/// The frame a target decodes, chosen by `mode % 3`: the input as it is, the input with its checksum repaired, or
/// the input as the body of a valid frame with `tag` and `version`.
#[must_use]
pub fn shape(mode: u8, tag: [u8; 4], version: u16, input: &[u8]) -> Vec<u8> {
    match mode % 3 {
        0 => input.to_vec(),
        1 => with_crc(input),
        _ => frame(tag, version, input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::codec::{Decoder, Encoder};

    #[test]
    fn test_frame_matches_the_encoder() {
        let mut enc = Encoder::new_frame(*b"TEST", 3);
        enc.put_u32(7);
        let body = 7u32.to_le_bytes();
        assert_eq!(frame(*b"TEST", 3, &body), enc.finish());
    }

    #[test]
    fn test_with_crc_repairs_a_damaged_checksum() {
        let mut bytes = frame(*b"TEST", 1, b"abcd");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        assert!(Decoder::open_frame(&bytes, *b"TEST", 1, 1).is_err());
        assert!(Decoder::open_frame(&with_crc(&bytes), *b"TEST", 1, 1).is_ok());
    }
}
