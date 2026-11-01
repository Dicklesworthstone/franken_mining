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

/// Nesting deeper than this is refused by [`is_json_line`], so the check cannot overflow the stack.
pub const MAX_JSON_DEPTH: usize = 128;

/// Whether `s` is exactly one JSON value (RFC 8259) and one line: no carriage return or line feed anywhere, as an
/// NDJSON envelope must be. Strings may hold no raw control character; escapes are the eight of RFC 8259 §7.
#[must_use]
pub fn is_json_line(s: &str) -> bool {
    let mut p = Json {
        b: s.as_bytes(),
        i: 0,
    };
    p.ws();
    let ok = p.value(0);
    p.ws();
    ok && p.i == p.b.len() && !s.contains(['\n', '\r'])
}

struct Json<'a> {
    b: &'a [u8],
    i: usize,
}

impl Json<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        let hit = self.peek() == Some(c);
        self.i += usize::from(hit);
        hit
    }

    fn lit(&mut self, word: &[u8]) -> bool {
        let hit = self.b[self.i..].starts_with(word);
        self.i += if hit { word.len() } else { 0 };
        hit
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        self.i - start
    }

    fn value(&mut self, depth: usize) -> bool {
        if depth > MAX_JSON_DEPTH {
            return false;
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string(),
            Some(b't') => self.lit(b"true"),
            Some(b'f') => self.lit(b"false"),
            Some(b'n') => self.lit(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => false,
        }
    }

    fn number(&mut self) -> bool {
        self.eat(b'-');
        if !self.eat(b'0') && self.digits() == 0 {
            return false;
        }
        if self.eat(b'.') && self.digits() == 0 {
            return false;
        }
        if self.eat(b'e') || self.eat(b'E') {
            let _ = self.eat(b'+') || self.eat(b'-');
            if self.digits() == 0 {
                return false;
            }
        }
        true
    }

    fn string(&mut self) -> bool {
        self.i += 1;
        loop {
            match self.peek() {
                None => return false,
                Some(b'"') => {
                    self.i += 1;
                    return true;
                }
                Some(0..=0x1F) => return false,
                Some(b'\\') => {
                    self.i += 1;
                    match self.peek() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => self.i += 1,
                        Some(b'u') => {
                            let hex = self.b.get(self.i + 1..self.i + 5);
                            if !hex.is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) {
                                return false;
                            }
                            self.i += 5;
                        }
                        _ => return false,
                    }
                }
                Some(_) => self.i += 1,
            }
        }
    }

    fn array(&mut self, depth: usize) -> bool {
        self.i += 1;
        self.ws();
        if self.eat(b']') {
            return true;
        }
        loop {
            self.ws();
            if !self.value(depth + 1) {
                return false;
            }
            self.ws();
            if self.eat(b']') {
                return true;
            }
            if !self.eat(b',') {
                return false;
            }
        }
    }

    fn object(&mut self, depth: usize) -> bool {
        self.i += 1;
        self.ws();
        if self.eat(b'}') {
            return true;
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') || !self.string() {
                return false;
            }
            self.ws();
            if !self.eat(b':') {
                return false;
            }
            self.ws();
            if !self.value(depth + 1) {
                return false;
            }
            self.ws();
            if self.eat(b'}') {
                return true;
            }
            if !self.eat(b',') {
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::codec::{Decoder, Encoder};

    #[test]
    fn test_is_json_line_accepts_rfc_8259_values() {
        for ok in [
            "0",
            "-0.5e+3",
            "1E9",
            "\"\"",
            "\"caf\u{e9} \\u00e9 \\\" \\\\ \\/ \\b \\f \\n \\r \\t\"",
            "[]",
            "{}",
            " {\"a\" : [1, true, false, null, {\"b\": \"\\ud800\"}]} ",
        ] {
            assert!(is_json_line(ok), "{ok:?}");
        }
    }

    #[test]
    fn test_is_json_line_refuses_what_rfc_8259_or_ndjson_refuse() {
        let deep = format!(
            "{}{}",
            "[".repeat(MAX_JSON_DEPTH + 2),
            "]".repeat(MAX_JSON_DEPTH + 2)
        );
        for bad in [
            "",
            "01",
            "1.",
            "-",
            ".5",
            "1e",
            "+1",
            "tru",
            "nul",
            "\"\u{1}\"",
            "\"\\q\"",
            "\"\\u12g4\"",
            "\"open",
            "[1 2]",
            "[1,]",
            "{\"a\":1,}",
            "{\"a\"}",
            "{a:1}",
            "{\"a\":1}{}",
            "{\"a\":1}\n",
            "[1,\n2]",
            &deep,
        ] {
            assert!(!is_json_line(bad), "{bad:?}");
        }
    }

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
