//! A bounded RFC 8259 JSON parser for OMF 2's index (`BD-194`). The workspace admits no `serde`, so the index is read
//! here.
//!
//! The text must be UTF-8. Nesting is bounded by `MAX_DEPTH`. A repeated member name, a lone surrogate, an unescaped
//! control character and anything after the value are refused, not repaired. Numbers keep the text they were written
//! with; `Json::f64` reads one correctly rounded, and `Json::u64` and `Json::i64` read whole numbers exactly.

/// The deepest nesting of arrays and objects accepted.
pub const MAX_DEPTH: usize = 128;

/// A JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// A number as written in the text.
    Number(String),
    String(String),
    Array(Vec<Json>),
    /// Members in the order written; names are unique.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The member named `key`, if this is an object that has one.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Self::Object(members) => Some(members),
            _ => None,
        }
    }

    /// A number, correctly rounded to `f64`; `None` for anything else or a magnitude past `f64::MAX`.
    #[must_use]
    pub fn f64(&self) -> Option<f64> {
        match self {
            Self::Number(text) => text.parse::<f64>().ok().filter(|x| x.is_finite()),
            _ => None,
        }
    }

    /// A number written as a whole number without sign, fraction or exponent, that fits `u64`.
    #[must_use]
    pub fn u64(&self) -> Option<u64> {
        match self {
            Self::Number(text) if text.bytes().all(|b| b.is_ascii_digit()) => text.parse().ok(),
            _ => None,
        }
    }

    /// A number written as a whole number without fraction or exponent, that fits `i64`.
    #[must_use]
    pub fn i64(&self) -> Option<i64> {
        match self {
            Self::Number(text)
                if text
                    .strip_prefix('-')
                    .unwrap_or(text)
                    .bytes()
                    .all(|b| b.is_ascii_digit()) =>
            {
                text.parse().ok()
            }
            _ => None,
        }
    }
}

/// The one JSON value in `bytes`, with only whitespace around it.
pub fn parse(bytes: &[u8]) -> Result<Json, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| format!("JSON is not UTF-8 at byte {}", e.valid_up_to()))?;
    let mut p = Parser {
        s: text.as_bytes(),
        pos: 0,
    };
    let value = p.value(0)?;
    p.space();
    if p.pos != p.s.len() {
        return Err(p.fail("text after the JSON value"));
    }
    Ok(value)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn fail(&self, what: &str) -> String {
        format!("JSON at byte {}: {what}", self.pos)
    }

    fn space(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.pos) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.fail(&format!("expected '{}'", b as char)))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        self.space();
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.literal(b"true", Json::Bool(true)),
            Some(b'f') => self.literal(b"false", Json::Bool(false)),
            Some(b'n') => self.literal(b"null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.fail("expected a value")),
            None => Err(self.fail("unexpected end of text")),
        }
    }

    fn literal(&mut self, word: &[u8], value: Json) -> Result<Json, String> {
        if self.s[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.fail("expected true, false or null"))
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.fail(&format!("nested deeper than {MAX_DEPTH}")));
        }
        self.pos += 1;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.space();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.space();
            if self.peek() != Some(b'"') {
                return Err(self.fail("expected a member name"));
            }
            let name = self.string()?;
            if members.iter().any(|(k, _)| *k == name) {
                return Err(self.fail(&format!("member \"{name}\" appears twice")));
            }
            self.space();
            self.expect(b':')?;
            let value = self.value(depth)?;
            members.push((name, value));
            self.space();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(members));
                }
                _ => return Err(self.fail("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.fail(&format!("nested deeper than {MAX_DEPTH}")));
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.space();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth)?);
            self.space();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.fail("expected ',' or ']'")),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if let Some(b'0'..=b'9') = self.peek() {
                    return Err(self.fail("a number has a leading zero"));
                }
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.fail("a number has no digits")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.fail("a fraction has no digits"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.fail("an exponent has no digits"));
            }
        }
        // The slice holds ASCII digits, signs, '.', 'e' and 'E' only.
        let text = String::from_utf8_lossy(&self.s[start..self.pos]).into_owned();
        Ok(Json::Number(text))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .s
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.fail("a \\u escape is truncated"))?;
        let mut v = 0_u32;
        for &d in digits {
            let n = (d as char)
                .to_digit(16)
                .ok_or_else(|| self.fail("a \\u escape has a non-hex digit"))?;
            v = v * 16 + n;
        }
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let b = self
                .peek()
                .ok_or_else(|| self.fail("a string is not terminated"))?;
            self.pos += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let e = self
                        .peek()
                        .ok_or_else(|| self.fail("an escape is truncated"))?;
                    self.pos += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
                        _ => return Err(self.fail("an unknown escape")),
                    };
                    let mut buf = [0_u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                0..=0x1F => return Err(self.fail("an unescaped control character in a string")),
                _ => out.push(b),
            }
        }
        // The input was checked as UTF-8 and escapes add whole characters, so this cannot fail.
        String::from_utf8(out).map_err(|_| self.fail("a string is not UTF-8"))
    }

    fn unicode(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        let code = match first {
            0xD800..=0xDBFF => {
                if !self.s[self.pos..].starts_with(b"\\u") {
                    return Err(self.fail("a high surrogate without its low surrogate"));
                }
                self.pos += 2;
                let low = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(self.fail("a high surrogate without its low surrogate"));
                }
                0x10000 + ((first - 0xD800) << 10) + (low - 0xDC00)
            }
            0xDC00..=0xDFFF => return Err(self.fail("a lone low surrogate")),
            _ => first,
        };
        char::from_u32(code).ok_or_else(|| self.fail("an escape that is not a character"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(s: &str) -> Json {
        Json::Number(s.to_owned())
    }

    #[test]
    fn values_parse_as_written() {
        let v = parse(
            br#" {"a": [1, -2.5e3, 0, true, false, null], "b": {"c": "x\"\\\/\b\f\n\r\t\u00e9\ud83d\ude00"}} "#,
        )
        .unwrap();
        assert_eq!(
            v.get("a").unwrap(),
            &Json::Array(vec![
                num("1"),
                num("-2.5e3"),
                num("0"),
                Json::Bool(true),
                Json::Bool(false),
                Json::Null
            ])
        );
        assert_eq!(
            v.get("b").and_then(|b| b.get("c")).and_then(Json::as_str),
            Some("x\"\\/\u{8}\u{c}\n\r\té\u{1F600}")
        );
        assert_eq!(v.get("z"), None);
        assert_eq!(parse(b"[]").unwrap(), Json::Array(vec![]));
        assert_eq!(parse(b"{}").unwrap(), Json::Object(vec![]));
        assert_eq!(
            parse("\"é\"".as_bytes()).unwrap(),
            Json::String("é".to_owned())
        );
    }

    #[test]
    fn numbers_read_exactly_or_not_at_all() {
        assert_eq!(num("0.1").f64(), Some(0.1));
        assert_eq!(num("-2.5e3").f64(), Some(-2500.0));
        assert_eq!(num("1e400").f64(), None);
        assert_eq!(num("18446744073709551615").u64(), Some(u64::MAX));
        assert_eq!(num("18446744073709551616").u64(), None);
        assert_eq!(num("-1").u64(), None);
        assert_eq!(num("1.0").u64(), None);
        assert_eq!(num("1e2").u64(), None);
        assert_eq!(num("-9223372036854775808").i64(), Some(i64::MIN));
        assert_eq!(num("9223372036854775808").i64(), None);
        assert_eq!(num("-1.5").i64(), None);
        assert_eq!(Json::Null.f64(), None);
    }

    #[test]
    fn bad_text_is_refused() {
        let refused = |text: &[u8], why: &str| {
            let e = parse(text).unwrap_err();
            assert!(e.contains(why), "{text:?}: {e}");
        };
        refused(b"", "unexpected end");
        refused(b"[1,]", "expected a value");
        refused(b"{\"a\":1,}", "member name");
        refused(b"{\"a\":1 \"b\":2}", "expected ',' or '}'");
        refused(b"[1 2]", "expected ',' or ']'");
        refused(b"{\"a\":1,\"a\":2}", "appears twice");
        refused(b"01", "leading zero");
        refused(b"-", "no digits");
        refused(b"1.", "fraction");
        refused(b"1e", "exponent");
        refused(b"+1", "expected a value");
        refused(b"tru", "true, false or null");
        refused(b"\"a", "not terminated");
        refused(b"\"a\tb\"", "control character");
        refused(b"\"\\x\"", "unknown escape");
        refused(b"\"\\u12zz\"", "non-hex");
        refused(b"\"\\u12", "truncated");
        refused(b"\"\\ud83d\"", "high surrogate");
        refused(b"\"\\ud83d\\u0041\"", "high surrogate");
        refused(b"\"\\ude00\"", "lone low surrogate");
        refused(b"1 2", "after the JSON value");
        refused(b"\"\xff\"", "not UTF-8");
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        refused(deep.as_bytes(), "nested deeper");
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(ok.as_bytes()).is_ok());
    }
}
