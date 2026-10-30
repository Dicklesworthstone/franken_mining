//! Displayed values of numeric cells (bd-jg43.2): the subset of the number-format grammar that the registered
//! files use, interpreted exactly on the stored decimal text. Pure functions, no I/O.
//!
//! A code splits on `;` outside quotes, escapes (`\x`), padding (`_x`), fill (`*x`) and brackets. It may have
//! up to four sections; the fourth is text and is not read. Each of the first three is read for the digit
//! placeholders `0 # ?`, the first `.`, `%` and quoted literals. Anything else that would change the value
//! refuses the whole code by name (exponent, letters, `/`, `@`, conditions, scaling commas, …). The zero
//! section, with no `0` placeholder and no `.`, shows its quoted literals as a token (`_(* "-"??_)` shows `-`).
//!
//! The section is chosen by the stored value: the second for a negative when there are two or more sections,
//! the third for zero when there are three or more, else the first. The value is the stored text as an exact
//! decimal, x100 for `%`, rounded half away from zero to 15 significant digits and then to the section's
//! decimals. HYPOTHESIS: the sections format |v|, and the minus comes from the negative section or, with one
//! section, from the value. Both roundings are also hypotheses about Excel; `scripts/oracle/as_shown_check.sh`
//! compares with LibreOffice's "as shown" text. Never via `f64`.

use crate::styles::NumberFormat;

/// A construct this interpreter does not support, named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

/// One numeric section of a format code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Digit placeholders after the decimal point.
    pub decimals: u8,
    /// An unquoted `%`: the value is shown x100.
    pub percent: bool,
    /// For a zero section with no `0` placeholder and no `.`: its quoted literals, shown instead of a number.
    pub zero_token: Option<String>,
}

/// The numeric sections of a format code (at most three) and how many sections the code has in all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatSpec {
    pub sections: Vec<Section>,
    pub section_count: usize,
}

/// A cell's displayed value. `scaled` is the signed value x 10^d; `negative` is set when a minus is shown, which
/// includes a negative that rounds to zero (`-0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Displayed {
    pub scaled: i128,
    pub d: u8,
    pub negative: bool,
    pub percent: bool,
    pub token: Option<String>,
}

impl Displayed {
    /// Canonical text: the token verbatim, or `-` when negative, the digits with exactly `d` decimals, and `%`
    /// for a percentage. No grouping, padding or brackets.
    #[must_use]
    pub fn text(&self) -> String {
        if let Some(token) = &self.token {
            return token.clone();
        }
        let digits = self.scaled.unsigned_abs().to_string();
        let d = usize::from(self.d);
        let padded = format!("{digits:0>width$}", width = d + 1);
        let (int, frac) = padded.split_at(padded.len() - d);
        let mut out = String::new();
        if self.negative {
            out.push('-');
        }
        out.push_str(int);
        if d > 0 {
            out.push('.');
            out.push_str(frac);
        }
        if self.percent {
            out.push('%');
        }
        out
    }
}

/// `general_non_integer`: General shows a non-integer at a precision that depends on the column width.
pub const GENERAL_NON_INTEGER: &str = "general_non_integer";
/// `general_too_wide`: General shows an integer of 10 or more digits by column width, possibly in E notation.
pub const GENERAL_TOO_WIDE: &str = "general_too_wide";
/// `unsupported_format`: the code uses a construct this interpreter refuses.
pub const UNSUPPORTED_FORMAT: &str = "unsupported_format";
/// `unknown_format_id`: the cell's `numFmtId` is neither listed nor built in.
pub const UNKNOWN_FORMAT_ID: &str = "unknown_format_id";
/// `bad_style_index`: the cell's `s` is past the end of `cellXfs`.
pub const BAD_STYLE_INDEX: &str = "bad_style_index";
/// `out_of_range`: the value x 10^d is 10^38 or more.
pub const OUT_OF_RANGE: &str = "out_of_range";
/// `bad_stored_text`: the stored text is not a decimal.
pub const BAD_STORED_TEXT: &str = "bad_stored_text";

/// Why a cell has no displayed value: one of the codes above and a reason naming the format code or value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRefusal {
    pub code: &'static str,
    pub reason: String,
}

fn refusal(code: &'static str, reason: String) -> DisplayRefusal {
    DisplayRefusal { code, reason }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Lit(String),
    Esc,
    Pad,
    Fill,
    Bracket(String),
    Ch(char),
}

fn unsupported(reason: impl Into<String>) -> Unsupported {
    Unsupported(reason.into())
}

fn split_sections(code: &str) -> Result<Vec<Vec<Token>>, Unsupported> {
    let chars: Vec<char> = code.chars().collect();
    let mut sections = Vec::new();
    let mut cur = Vec::new();
    let mut i = 0;
    while let Some(&ch) = chars.get(i) {
        match ch {
            '"' => {
                let rest = chars.get(i + 1..).unwrap_or_default();
                let j = rest
                    .iter()
                    .position(|&c| c == '"')
                    .ok_or_else(|| unsupported("unterminated quote"))?;
                cur.push(Token::Lit(rest[..j].iter().collect()));
                i += j + 2;
            }
            '\\' | '_' | '*' => {
                if chars.get(i + 1).is_none() {
                    return Err(unsupported(format!("dangling {ch}")));
                }
                cur.push(match ch {
                    '\\' => Token::Esc,
                    '_' => Token::Pad,
                    _ => Token::Fill,
                });
                i += 2;
            }
            '[' => {
                let rest = chars.get(i + 1..).unwrap_or_default();
                let j = rest
                    .iter()
                    .position(|&c| c == ']')
                    .ok_or_else(|| unsupported("unterminated bracket"))?;
                cur.push(Token::Bracket(rest[..j].iter().collect()));
                i += j + 2;
            }
            ';' => {
                sections.push(std::mem::take(&mut cur));
                i += 1;
            }
            _ => {
                cur.push(Token::Ch(ch));
                i += 1;
            }
        }
    }
    sections.push(cur);
    Ok(sections)
}

const COLOURS: [&str; 8] = [
    "black", "blue", "cyan", "green", "magenta", "red", "white", "yellow",
];

fn read_section(tokens: &[Token], index: usize) -> Result<Section, Unsupported> {
    if tokens.is_empty() {
        return Err(unsupported("empty section"));
    }
    let (mut decimals, mut percent, mut has_zero, mut point) = (0usize, false, false, false);
    let mut literal = String::new();
    let mut last_placeholder: Option<usize> = None;
    let mut commas = Vec::new();
    for (k, token) in tokens.iter().enumerate() {
        match token {
            Token::Lit(s) => literal.push_str(s),
            Token::Bracket(b) => {
                if b.starts_with(['<', '>', '=']) {
                    return Err(unsupported(format!("condition [{b}]")));
                }
                let low = b.to_lowercase();
                let colour = COLOURS.contains(&low.as_str())
                    || low
                        .strip_prefix("color")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()));
                if !colour {
                    return Err(unsupported(format!("bracket [{b}]")));
                }
            }
            Token::Ch(c) => match *c {
                '0' | '#' | '?' => {
                    last_placeholder = Some(k);
                    has_zero |= *c == '0';
                    decimals += usize::from(point);
                }
                '.' => {
                    if point {
                        return Err(unsupported("second decimal point"));
                    }
                    point = true;
                }
                '%' => {
                    if percent {
                        return Err(unsupported("second %"));
                    }
                    percent = true;
                }
                ',' => commas.push(k),
                'e' | 'E' => return Err(unsupported("scientific notation")),
                c if c.is_ascii_alphabetic() => {
                    return Err(unsupported(format!("letter {c} (date, time or keyword)")));
                }
                '/' => return Err(unsupported("fraction")),
                '@' => return Err(unsupported("text placeholder")),
                _ => {}
            },
            Token::Esc | Token::Pad | Token::Fill => {}
        }
    }
    if commas
        .iter()
        .any(|&k| last_placeholder.is_none_or(|p| k > p))
    {
        return Err(unsupported("scaling comma"));
    }
    let decimals = u8::try_from(decimals)
        .ok()
        .filter(|&d| d <= 30)
        .ok_or_else(|| unsupported("more than 30 decimals"))?;
    if index == 2 && !has_zero && !point {
        if literal.is_empty() {
            return Err(unsupported("zero section shows no digit or literal"));
        }
        return Ok(Section {
            decimals,
            percent,
            zero_token: Some(literal),
        });
    }
    if !has_zero {
        return Err(unsupported("no 0 placeholder"));
    }
    Ok(Section {
        decimals,
        percent,
        zero_token: None,
    })
}

/// Parses a format code into its numeric sections, or names the first construct that is not supported.
pub fn parse_format(code: &str) -> Result<FormatSpec, Unsupported> {
    let sections = split_sections(code)?;
    if sections.len() > 4 {
        return Err(unsupported("more than four sections"));
    }
    let numeric = sections
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, s)| read_section(s, i))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FormatSpec {
        sections: numeric,
        section_count: sections.len(),
    })
}

/// An exact decimal: `digits` (no leading zero; empty for zero) x 10^`exp`, with a sign.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decimal {
    negative: bool,
    digits: Vec<u8>,
    exp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sign {
    Negative,
    Zero,
    Positive,
}

impl Decimal {
    /// `[+-]digits[.digits][(e|E)[+-]digits]`, with at least one digit before the exponent.
    fn parse(text: &str) -> Option<Self> {
        let t = text.trim();
        let (negative, t) = match t.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        let (mantissa, exp) = match t.find(['e', 'E']) {
            Some(i) => {
                let e = &t[i + 1..];
                let body = e.strip_prefix(['+', '-']).unwrap_or(e);
                if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let saturated = if e.starts_with('-') {
                    i64::MIN / 4
                } else {
                    i64::MAX / 4
                };
                (&t[..i], e.parse::<i64>().unwrap_or(saturated))
            }
            None => (t, 0),
        };
        let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if (int.is_empty() && frac.is_empty())
            || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let digits: Vec<u8> = int
            .bytes()
            .chain(frac.bytes())
            .map(|b| b - b'0')
            .skip_while(|&d| d == 0)
            .collect();
        let frac_len = i64::try_from(frac.len()).ok()?;
        Some(Self {
            negative: negative && !digits.is_empty(),
            digits,
            exp: exp.saturating_sub(frac_len),
        })
    }

    fn sign(&self) -> Sign {
        if self.digits.is_empty() {
            Sign::Zero
        } else if self.negative {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }
}

/// Adds one to a digit string, carrying.
fn increment(digits: &mut Vec<u8>) {
    for d in digits.iter_mut().rev() {
        if *d == 9 {
            *d = 0;
        } else {
            *d += 1;
            return;
        }
    }
    digits.insert(0, 1);
}

/// Rounds to at most `sig` significant digits; `round_up(next digit)` decides the last kept digit.
fn round_significant(
    digits: &[u8],
    exp: i64,
    sig: usize,
    round_up: fn(u8) -> bool,
) -> (Vec<u8>, i64) {
    let Some(&next) = digits.get(sig) else {
        return (digits.to_vec(), exp);
    };
    let mut kept = digits[..sig].to_vec();
    if round_up(next) {
        increment(&mut kept);
    }
    let dropped = i64::try_from(digits.len() - sig).unwrap_or(i64::MAX);
    (kept, exp.saturating_add(dropped))
}

const BOUND: u128 = 10u128.pow(38);

/// `digits x 10^exp x 10^d` rounded to an integer, or `None` at 10^38 or more.
fn scaled_at(digits: &[u8], exp: i64, d: u8, round_up: fn(u8) -> bool) -> Option<u128> {
    let k = exp.saturating_add(i64::from(d));
    let to_int = |ds: &[u8]| {
        ds.iter().try_fold(0u128, |acc, &x| {
            acc.checked_mul(10)
                .and_then(|a| a.checked_add(u128::from(x)))
                .filter(|&a| a < BOUND)
        })
    };
    if k >= 0 {
        let mut v = to_int(digits)?;
        for _ in 0..k {
            if v == 0 {
                break;
            }
            v = v.checked_mul(10).filter(|&a| a < BOUND)?;
        }
        return Some(v);
    }
    let len = i64::try_from(digits.len()).ok()?;
    let cut = len + k;
    if cut < 0 {
        return Some(0);
    }
    let cut = usize::try_from(cut).ok()?;
    let mut v = to_int(&digits[..cut])?;
    if digits.get(cut).is_some_and(|&n| round_up(n)) {
        v = v.checked_add(1).filter(|&a| a < BOUND)?;
    }
    Some(v)
}

fn pick_section(section_count: usize, sign: Sign) -> usize {
    match sign {
        Sign::Negative if section_count >= 2 => 1,
        Sign::Zero if section_count >= 3 => 2,
        _ => 0,
    }
}

fn half_away_from_zero(next: u8) -> bool {
    next >= 5
}

fn display_spec(
    v: &Decimal,
    spec: &FormatSpec,
    pick: fn(usize, Sign) -> usize,
    round_up: fn(u8) -> bool,
) -> Result<Displayed, DisplayRefusal> {
    let sign = v.sign();
    let idx = pick(spec.section_count, sign);
    let section = spec.sections.get(idx).ok_or_else(|| {
        refusal(
            UNSUPPORTED_FORMAT,
            format!("section {idx} of {} is not numeric", spec.section_count),
        )
    })?;
    if let Some(token) = &section.zero_token {
        return Ok(Displayed {
            scaled: 0,
            d: section.decimals,
            negative: false,
            percent: section.percent,
            token: Some(token.clone()),
        });
    }
    let negative = idx == 1 || (spec.section_count == 1 && sign == Sign::Negative);
    let exp = if section.percent {
        v.exp.saturating_add(2)
    } else {
        v.exp
    };
    let (digits, exp) = round_significant(&v.digits, exp, 15, round_up);
    let magnitude = scaled_at(&digits, exp, section.decimals, round_up)
        .and_then(|m| i128::try_from(m).ok())
        .ok_or_else(|| refusal(OUT_OF_RANGE, "value x 10^d is 10^38 or more".to_string()))?;
    Ok(Displayed {
        scaled: if negative { -magnitude } else { magnitude },
        d: section.decimals,
        negative,
        percent: section.percent,
        token: None,
    })
}

fn display_general(v: &Decimal) -> Result<Displayed, DisplayRefusal> {
    let trailing = v.digits.iter().rev().take_while(|&&d| d == 0).count();
    let digits = &v.digits[..v.digits.len() - trailing];
    let exp = v
        .exp
        .saturating_add(i64::try_from(trailing).unwrap_or(i64::MAX));
    if !digits.is_empty() && exp < 0 {
        return Err(refusal(
            GENERAL_NON_INTEGER,
            "General shows a non-integer by column width".to_string(),
        ));
    }
    let width = i64::try_from(digits.len())
        .unwrap_or(i64::MAX)
        .saturating_add(exp);
    if !digits.is_empty() && width > 10 {
        return Err(refusal(
            GENERAL_TOO_WIDE,
            "General shows an integer of 10 or more digits by column width".to_string(),
        ));
    }
    let magnitude = scaled_at(digits, exp, 0, half_away_from_zero)
        .and_then(|m| i128::try_from(m).ok())
        .ok_or_else(|| refusal(OUT_OF_RANGE, "integer is 10^38 or more".to_string()))?;
    Ok(Displayed {
        scaled: if v.negative { -magnitude } else { magnitude },
        d: 0,
        negative: v.negative,
        percent: false,
        token: None,
    })
}

/// The displayed value of a numeric cell whose stored text is `raw` and whose format is `fmt`.
pub fn display(raw: &str, fmt: &NumberFormat) -> Result<Displayed, DisplayRefusal> {
    let code = match fmt {
        NumberFormat::BadStyleIndex(s) => {
            return Err(refusal(
                BAD_STYLE_INDEX,
                format!("style index {s} is past cellXfs"),
            ));
        }
        NumberFormat::Unknown(id) => {
            return Err(refusal(
                UNKNOWN_FORMAT_ID,
                format!("numFmtId {id} is neither listed nor built in"),
            ));
        }
        NumberFormat::Builtin(_) | NumberFormat::Custom { .. } => fmt.code(),
    };
    let v = Decimal::parse(raw)
        .ok_or_else(|| refusal(BAD_STORED_TEXT, format!("{raw:?} is not a decimal")))?;
    if code.eq_ignore_ascii_case("general") {
        return display_general(&v);
    }
    let spec = parse_format(&code)
        .map_err(|u| refusal(UNSUPPORTED_FORMAT, format!("{code:?}: {}", u.0)))?;
    display_spec(&v, &spec, pick_section, half_away_from_zero)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNTING: &str = r#"_(* #,##0_);_(* \(#,##0\);_(* "-"??_);_(@_)"#;
    const ACCOUNTING_DASH: &str = r#"_(* #,##0_);_(* \(#,##0\);_(* "-"_);_(@_)"#;
    const EM_DASH: &str = r#"_(#,##0_);_(\(#,##0\);_("—"_);_(@_)"#;

    fn custom(code: &str) -> NumberFormat {
        NumberFormat::Custom {
            id: 164,
            code: code.to_string(),
        }
    }

    fn shown(raw: &str, fmt: &NumberFormat) -> String {
        display(raw, fmt).map_or_else(|r| format!("REFUSED:{}", r.code), |d| d.text())
    }

    #[test]
    fn test_split_sections_respects_quoted_semicolon() {
        let s = split_sections(r#"0;"a;b"0;0"#).expect("split");
        assert_eq!(s.len(), 3);
        assert_eq!(s[1][0], Token::Lit("a;b".to_string()));
        assert_eq!(
            split_sections(r"0\;0").expect("split").len(),
            1,
            "escaped ;"
        );
        assert_eq!(split_sections("0_;0").expect("split").len(), 1, "padding ;");
        assert!(split_sections(r#"0;"open"#).is_err());
    }

    #[test]
    fn test_escapes_padding_fill_color_ignored() {
        let spec = parse_format(r#"[Red]_(* \$#,##0.0_);[Color10]\(0.0\)"#).expect("spec");
        assert_eq!(spec.section_count, 2);
        for s in &spec.sections {
            assert_eq!(
                (s.decimals, s.percent, s.zero_token.clone()),
                (1, false, None)
            );
        }
        // An escaped or padded letter is not a date letter, nor an escaped 0 a placeholder.
        assert!(parse_format(r"0\d_m*y").is_ok());
        assert!(parse_format(r"\0").is_err(), "no 0 placeholder");
    }

    #[test]
    fn test_decimals_and_percent_for_all_ten_codes() {
        // S1's ten codes (E833). General is not a section format and is handled apart.
        for (code, decimals, percent) in [
            ("0", 0, false),
            ("0.00", 2, false),
            ("#,##0", 0, false),
            ("#,##0.00", 2, false),
            ("0%", 0, true),
            ("0.00%", 2, true),
            (ACCOUNTING, 0, false),
            (ACCOUNTING_DASH, 0, false),
            (EM_DASH, 0, false),
        ] {
            let spec = parse_format(code).expect(code);
            assert_eq!(
                (spec.sections[0].decimals, spec.sections[0].percent),
                (decimals, percent),
                "{code}"
            );
        }
        assert_eq!(shown("12", &NumberFormat::Builtin(0)), "12");
    }

    #[test]
    fn test_zero_section_dash_with_question_padding_is_token() {
        for (code, token) in [(ACCOUNTING, "-"), (ACCOUNTING_DASH, "-"), (EM_DASH, "—")] {
            let spec = parse_format(code).expect(code);
            assert_eq!(spec.section_count, 4);
            assert_eq!(
                spec.sections[2].zero_token.as_deref(),
                Some(token),
                "{code}"
            );
            let d = display("0", &custom(code)).expect("zero");
            assert_eq!((d.text(), d.scaled), (token.to_string(), 0));
        }
    }

    /// E823 K8.
    #[test]
    fn test_k8_minus_37_899642999999998_displays_minus_38() {
        let d = display("-37.899642999999998", &custom(ACCOUNTING)).expect("K8");
        assert_eq!(
            (d.scaled, d.d, d.negative, d.text()),
            (-38, 0, true, "-38".to_string())
        );
        assert_eq!(shown("132.100357", &custom(ACCOUNTING)), "132");
        assert_eq!(shown("133.100357", &custom(ACCOUNTING)), "133");
    }

    #[test]
    fn test_round_0_125_two_dp() {
        let d = display("0.125", &NumberFormat::Builtin(2)).expect("0.125");
        assert_eq!((d.scaled, d.text()), (13, "0.13".to_string()));
    }

    /// HYPOTHESIS: Excel rounds to 15 significant digits first, so the stored 1.0049999999999999 (the double
    /// nearest 1.005) shows 1.01, not 1.00.
    #[test]
    fn test_round_1_0049999999999999_two_dp_hypothesis() {
        assert_eq!(
            shown("1.0049999999999999", &NumberFormat::Builtin(2)),
            "1.01"
        );
        assert_eq!(shown("1.00499999999999", &NumberFormat::Builtin(2)), "1.00");
    }

    #[test]
    fn test_half_cases_2_5_and_minus_2_5() {
        assert_eq!(shown("2.5", &NumberFormat::Builtin(1)), "3");
        assert_eq!(shown("-2.5", &NumberFormat::Builtin(1)), "-3");
        assert_eq!(shown("0.005", &NumberFormat::Builtin(10)), "0.50%");
        assert_eq!(shown("0.12345", &NumberFormat::Builtin(10)), "12.35%");
    }

    /// HYPOTHESIS: the section follows the stored sign, so a negative that rounds to zero still shows a minus.
    #[test]
    fn test_section_chosen_by_stored_sign_hypothesis() {
        let two = custom(r"#,##0_);\(#,##0\)");
        let d = display("-0.3", &two).expect("-0.3");
        assert_eq!(
            (d.scaled, d.negative, d.text()),
            (0, true, "-0".to_string())
        );
        assert_eq!(shown("-0.3", &NumberFormat::Builtin(1)), "-0");
        // A stored 0.0001 is not zero: the first section shows 0, not the zero token.
        assert_eq!(shown("0.0001", &custom(ACCOUNTING)), "0");
        assert_eq!(shown("-0", &custom(ACCOUNTING)), "-");
    }

    #[test]
    fn test_unsupported_formats_refuse_by_name() {
        for (code, reason) in [
            ("m/d/yyyy", "letter m"),
            ("0.00E+00", "scientific"),
            ("# ?/?", "fraction"),
            ("[>100]0", "condition"),
            ("#,##0,", "scaling comma"),
            ("@", "text placeholder"),
            ("0;0;0;@;0", "more than four"),
            ("0.00.0", "second decimal point"),
            ("[$-409]0", "bracket"),
        ] {
            let err = display("1", &custom(code)).expect_err(code);
            assert_eq!(err.code, UNSUPPORTED_FORMAT, "{code}");
            assert!(err.reason.contains(code), "{code}: {}", err.reason);
            assert!(err.reason.contains(reason), "{code}: {}", err.reason);
        }
        assert_eq!(
            display("1", &NumberFormat::Unknown(200)).map_err(|e| e.code),
            Err(UNKNOWN_FORMAT_ID)
        );
        assert_eq!(
            display("1", &NumberFormat::BadStyleIndex(9)).map_err(|e| e.code),
            Err(BAD_STYLE_INDEX)
        );
    }

    #[test]
    fn test_general_non_integer_refused() {
        let general = NumberFormat::Builtin(0);
        assert_eq!(shown("2.5", &general), "REFUSED:general_non_integer");
        assert_eq!(shown("2026", &general), "2026");
        assert_eq!(shown("-1.5E+1", &general), "-15");
        assert_eq!(shown("12.000", &general), "12");
        assert_eq!(shown("9999999999", &general), "9999999999");
        assert_eq!(shown("10000000000", &general), "REFUSED:general_too_wide");
        assert_eq!(shown("x", &general), "REFUSED:bad_stored_text");
        assert_eq!(
            shown("1E+40", &NumberFormat::Builtin(3)),
            "REFUSED:out_of_range"
        );
    }

    /// A seeded sweep (no proptest dependency): an integer stored in any spelling displays as itself under
    /// every supported d = 0 code.
    #[test]
    fn prop_integer_stored_equals_displayed_for_d0() {
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        let codes = [
            NumberFormat::Builtin(1),
            NumberFormat::Builtin(3),
            custom(ACCOUNTING),
            custom(EM_DASH),
        ];
        for _ in 0..5_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let magnitude = x % 10_000_000_000;
            let negative = x & 1 == 1 && magnitude != 0;
            let spelled = match (x >> 8) % 3 {
                0 => magnitude.to_string(),
                1 => format!("{magnitude}.000"),
                _ => format!("{}E+1", magnitude as f64 / 10.0),
            };
            let raw = if negative {
                format!("-{spelled}")
            } else {
                spelled
            };
            let want = if negative {
                format!("-{magnitude}")
            } else {
                magnitude.to_string()
            };
            for fmt in &codes {
                let got = shown(&raw, fmt);
                if magnitude == 0 && matches!(fmt, NumberFormat::Custom { .. }) {
                    assert!(got == "-" || got == "—", "{raw} {fmt:?}: {got}");
                } else {
                    assert_eq!(got, want, "{raw} {fmt:?}");
                }
            }
        }
    }

    /// Mutation: choosing the first section for negatives loses K8's minus.
    #[test]
    fn test_mutation_ignore_negative_section_fails_k8() {
        let v = Decimal::parse("-37.899642999999998").expect("K8");
        let spec = parse_format(ACCOUNTING).expect("spec");
        let mutant = display_spec(&v, &spec, |_, _| 0, half_away_from_zero).expect("mutant");
        assert_ne!(mutant.text(), "-38");
        let real = display_spec(&v, &spec, pick_section, half_away_from_zero).expect("real");
        assert_eq!(real.text(), "-38");
    }

    /// Mutation: truncating instead of rounding shows K8 as -37.
    #[test]
    fn test_mutation_truncate_instead_of_round_fails_k8() {
        let v = Decimal::parse("-37.899642999999998").expect("K8");
        let spec = parse_format(ACCOUNTING).expect("spec");
        let mutant = display_spec(&v, &spec, pick_section, |_| false).expect("mutant");
        assert_eq!(mutant.text(), "-37");
    }
}
