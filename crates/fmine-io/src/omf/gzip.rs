//! RFC 1952 gzip members over the crate's RFC 1951 inflater (`BD-194`). OMF 2 gzips its JSON index, and Parquet's
//! GZIP codec is a gzip member per page.
//!
//! One member only, inflated to exactly the size its trailer declares, and no more than the caller's bound; the
//! CRC-32 in the trailer is checked, and so is the header CRC when the header carries one.

pub(crate) use crate::zip_inflate::crc32;
use crate::zip_inflate::inflate_raw;

const FHCRC: u8 = 0x02;
const FEXTRA: u8 = 0x04;
const FNAME: u8 = 0x08;
const FCOMMENT: u8 = 0x10;
const RESERVED: u8 = 0xE0;

/// The bytes of one gzip member, which must inflate to at most `max` bytes.
pub(crate) fn gunzip(bytes: &[u8], max: usize) -> Result<Vec<u8>, String> {
    if bytes.len() < 18 {
        return Err(format!("gzip member of {} bytes is too short", bytes.len()));
    }
    if bytes[0] != 0x1F || bytes[1] != 0x8B {
        return Err("not a gzip member (bad magic)".to_owned());
    }
    if bytes[2] != 8 {
        return Err(format!(
            "gzip compression method {}, not DEFLATE (8)",
            bytes[2]
        ));
    }
    let flags = bytes[3];
    if flags & RESERVED != 0 {
        return Err(format!("gzip header sets reserved flags {flags:#04x}"));
    }
    let body_end = bytes.len() - 8;
    let mut pos = 10;
    if flags & FEXTRA != 0 {
        let len = bytes
            .get(pos..pos + 2)
            .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
            .ok_or("gzip extra field is truncated")?;
        pos += 2 + len;
    }
    for flag in [FNAME, FCOMMENT] {
        if flags & flag != 0 {
            let end = bytes
                .get(pos..body_end)
                .and_then(|rest| rest.iter().position(|&b| b == 0))
                .ok_or("gzip name or comment is not terminated")?;
            pos += end + 1;
        }
    }
    if flags & FHCRC != 0 {
        let stored = bytes
            .get(pos..pos + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or("gzip header CRC is truncated")?;
        if stored != crc32(&bytes[..pos]) as u16 {
            return Err("gzip header CRC does not match".to_owned());
        }
        pos += 2;
    }
    if pos > body_end {
        return Err("gzip header runs into the trailer".to_owned());
    }
    let trailer = &bytes[body_end..];
    let crc = u32::from_le_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    let size = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]) as usize;
    if size > max {
        return Err(format!(
            "gzip member declares {size} bytes, more than the {max} allowed"
        ));
    }
    let out = inflate_raw(&bytes[pos..body_end], size).map_err(|e| format!("gzip: {e}"))?;
    if crc32(&out) != crc {
        return Err("gzip CRC-32 does not match the inflated bytes".to_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `gzip.compress(b"hello, world\n", mtime=0)` from CPython 3.15.0's `gzip` module, an independent encoder.
    const HELLO: [u8; 33] = [
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xcb, 0x48, 0xcd, 0xc9, 0xc9,
        0xd7, 0x51, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0xe1, 0x02, 0x00, 0x53, 0x74, 0x24, 0xf4, 0x0d,
        0x00, 0x00, 0x00,
    ];

    #[test]
    fn crc32_matches_the_check_value() {
        // The CRC-32 check value: the CRC of the nine ASCII digits "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn a_member_from_another_encoder_inflates() {
        assert_eq!(gunzip(&HELLO, 64).unwrap(), b"hello, world\n");
    }

    #[test]
    fn a_name_and_a_header_crc_are_read() {
        // The same member with FNAME "a" and FHCRC set: the header is rebuilt and its CRC computed here.
        let mut header = vec![
            0x1f,
            0x8b,
            0x08,
            FNAME | FHCRC,
            0,
            0,
            0,
            0,
            0x00,
            0xff,
            b'a',
            0,
        ];
        let crc = crc32(&header) as u16;
        header.extend_from_slice(&crc.to_le_bytes());
        header.extend_from_slice(&HELLO[10..]);
        assert_eq!(gunzip(&header, 64).unwrap(), b"hello, world\n");
        let last = header.len() - 1;
        header[13] ^= 1;
        assert!(gunzip(&header, 64).unwrap_err().contains("header CRC"));
        header[13] ^= 1;
        header[last] = 1;
        assert!(gunzip(&header, 1 << 30).is_err());
    }

    #[test]
    fn a_bad_member_is_refused() {
        let refused = |bytes: &[u8], max: usize, why: &str| {
            let e = gunzip(bytes, max).unwrap_err();
            assert!(e.contains(why), "{e}");
        };
        refused(&HELLO[..17], 64, "too short");
        let mut b = HELLO;
        b[0] = 0;
        refused(&b, 64, "magic");
        let mut b = HELLO;
        b[2] = 7;
        refused(&b, 64, "method");
        let mut b = HELLO;
        b[3] = 0x20;
        refused(&b, 64, "reserved");
        // The CRC in the trailer.
        let mut b = HELLO;
        b[25] ^= 1;
        refused(&b, 64, "CRC-32");
        // The size in the trailer: more than the stream holds, then less.
        let mut b = HELLO;
        b[29] = 14;
        refused(&b, 64, "declared size");
        let mut b = HELLO;
        b[29] = 12;
        refused(&b, 64, "declared size");
        // The caller's bound.
        refused(&HELLO, 12, "more than the 12 allowed");
    }
}
