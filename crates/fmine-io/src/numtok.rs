//! Exact lexical inputs for cited disclosure lint (BD-345.S1). Tokens describe printed syntax, never infer
//! a metric, ownership basis or truth. Offsets are UTF-8 byte boundaries into the unchanged input. Numeric
//! precision comes from printed digits; neither a hedge nor a trailing zero invents a wider tolerance.

pub type Span = (usize, usize);

pub mod equality;

/// `mantissa / 10^scale`, without floating point. Trailing decimal places remain significant in `step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dec {
    pub mantissa: i128,
    pub scale: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleWord {
    Thousand,
    Million,
    Billion,
}

impl ScaleWord {
    const fn power(self) -> u32 {
        match self {
            Self::Thousand => 3,
            Self::Million => 6,
            Self::Billion => 9,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Usd,
    Cad,
    Aud,
    Eur,
    Gbp,
    Dollar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitTok {
    UsdPerOz,
    UsdPerLb,
    UsdPerTonne,
    DollarPerOz,
    DollarPerLb,
    Oz,
    Lb,
    Tonne,
    Koz,
    Moz,
    Kt,
    GramsPerTonne,
    Percent,
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasisTok {
    Attributable,
    Hundred,
    Unprinted,
    PerOzSold,
    PerOzProduced,
    PerLbSold,
    PerLbProduced,
    ByProduct,
    CoProduct,
    GoldOunces,
    Geo,
    NonGaap,
    Scorecard,
}

impl BasisTok {
    #[must_use]
    pub const fn is_ownership(self) -> bool {
        matches!(self, Self::Attributable | Self::Hundred | Self::Unprinted)
    }

    #[must_use]
    pub const fn is_denominator(self) -> bool {
        matches!(
            self,
            Self::PerOzSold | Self::PerOzProduced | Self::PerLbSold | Self::PerLbProduced
        )
    }

    #[must_use]
    pub const fn is_credit(self) -> bool {
        matches!(self, Self::ByProduct | Self::CoProduct)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodTok {
    Quarter { q: u8, year: Option<u16> },
    Half { h: u8, year: Option<u16> },
    Fy { year: u16 },
    YearRange { from: u16, to: u16 },
    ThreeYear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hedge {
    About,
    CloseTo,
    Approximately,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberToken {
    pub span: Span,
    pub raw: String,
    pub value: Dec,
    pub step: Dec,
    pub scale_word: Option<ScaleWord>,
    pub currency: Option<Currency>,
    pub unit: Option<UnitTok>,
    /// Values and steps of percentages stay in percentage points, not fractions.
    pub percent: bool,
    pub paren_negative: bool,
    pub hedge: Option<Hedge>,
    pub range_with: Option<Dec>,
    pub range_step: Option<Dec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Number(NumberToken),
    Basis {
        span: Span,
        kind: BasisTok,
    },
    Period {
        span: Span,
        kind: PeriodTok,
    },
    Unit {
        span: Span,
        kind: UnitTok,
    },
    /// A number-shaped input outside the exact grammar remains visible to a fail-closed consumer.
    UnsupportedNumber {
        span: Span,
        raw: String,
        reason: &'static str,
    },
}

impl Token {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Number(n) => n.span,
            Self::Basis { span, .. }
            | Self::Period { span, .. }
            | Self::Unit { span, .. }
            | Self::UnsupportedNumber { span, .. } => *span,
        }
    }
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
fn after_word(text: &str, at: usize) -> bool {
    text.get(at..)
        .and_then(|s| s.chars().next())
        .is_some_and(word)
}
fn start_boundary(text: &str, at: usize) -> bool {
    text.get(..at)
        .and_then(|s| s.chars().next_back())
        .is_none_or(|c| !word(c) && c != '.' && c != ',')
}
fn spaces(text: &str, mut at: usize) -> usize {
    while let Some(c) = text
        .get(at..)
        .and_then(|s| s.chars().next())
        .filter(|c| c.is_whitespace())
    {
        at += c.len_utf8();
    }
    at
}
fn prefix(text: &str, at: usize, literal: &str) -> Option<usize> {
    let end = at.checked_add(literal.len())?;
    text.get(at..end)
        .filter(|s| s.eq_ignore_ascii_case(literal))
        .map(|_| end)
}
/// Matches registered words, allowing whitespace variation but never a substring of a longer word.
fn phrase(text: &str, at: usize, literal: &str) -> Option<usize> {
    let mut end = at;
    for (i, part) in literal.split(' ').enumerate() {
        if i != 0 {
            let next = spaces(text, end);
            if next == end {
                return None;
            }
            end = next;
        }
        end = prefix(text, end, part)?;
    }
    (!after_word(text, end)).then_some(end)
}

fn basis(text: &str, at: usize) -> Option<(usize, BasisTok)> {
    use BasisTok as B;
    [
        ("ownership basis unprinted", B::Unprinted),
        ("unprinted ownership", B::Unprinted),
        ("compensation-adjusted", B::Scorecard),
        ("api scorecard", B::Scorecard),
        ("per ounce produced", B::PerOzProduced),
        ("per oz produced", B::PerOzProduced),
        ("per ounce sold", B::PerOzSold),
        ("per oz sold", B::PerOzSold),
        ("per pound produced", B::PerLbProduced),
        ("per lb produced", B::PerLbProduced),
        ("per pound sold", B::PerLbSold),
        ("per lb sold", B::PerLbSold),
        ("gold equivalent", B::Geo),
        ("gold-equivalent", B::Geo),
        ("geo", B::Geo),
        ("gold ounces", B::GoldOunces),
        ("ounces of gold", B::GoldOunces),
        ("100% basis", B::Hundred),
        ("100 % basis", B::Hundred),
        ("100 percent basis", B::Hundred),
        ("consolidated", B::Hundred),
        ("attributable", B::Attributable),
        ("equity share", B::Attributable),
        ("by-product", B::ByProduct),
        ("by product", B::ByProduct),
        ("co-product", B::CoProduct),
        ("co product", B::CoProduct),
        ("coproduct", B::CoProduct),
        ("non-gaap", B::NonGaap),
    ]
    .into_iter()
    .find_map(|(p, k)| phrase(text, at, p).map(|end| (end, k)))
}

fn four_year(text: &str, at: usize) -> Option<(usize, u16)> {
    let end = at.checked_add(4)?;
    let value = text.get(at..end)?;
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year = value.parse::<u16>().ok()?;
    (year >= 1000).then_some((end, year))
}
fn following_year(text: &str, at: usize) -> (usize, Option<u16>) {
    let mut next = spaces(text, at);
    if next == at {
        return (at, None);
    }
    if let Some(end) = phrase(text, next, "of") {
        next = spaces(text, end);
    }
    match four_year(text, next).filter(|(end, _)| !after_word(text, *end)) {
        Some((end, year)) => (end, Some(year)),
        None => (at, None),
    }
}

fn period(text: &str, at: usize) -> Option<(usize, PeriodTok)> {
    for p in ["three-year", "three year"] {
        if let Some(end) = phrase(text, at, p) {
            return Some((end, PeriodTok::ThreeYear));
        }
    }
    for (p, h) in [("first half", 1), ("second half", 2)] {
        if let Some(end) = phrase(text, at, p) {
            let (end, year) = following_year(text, end);
            return Some((end, PeriodTok::Half { h, year }));
        }
    }
    if let Some(end) = prefix(text, at, "FY")
        && let Some((end, year)) =
            four_year(text, spaces(text, end)).filter(|(end, _)| !after_word(text, *end))
    {
        return Some((end, PeriodTok::Fy { year }));
    }
    for q in 1..=4 {
        if let Some(end) = phrase(text, at, &format!("Q{q}")) {
            let (end, year) = following_year(text, end);
            return Some((end, PeriodTok::Quarter { q, year }));
        }
    }
    for h in 1..=2 {
        if let Some(end) = phrase(text, at, &format!("H{h}")) {
            let (end, year) = following_year(text, end);
            return Some((end, PeriodTok::Half { h, year }));
        }
    }
    let (end, year) = four_year(text, at)?;
    let next = spaces(text, end);
    for q in 1..=4 {
        if let Some(end) = phrase(text, next, &format!("Q{q}")) {
            return Some((
                end,
                PeriodTok::Quarter {
                    q,
                    year: Some(year),
                },
            ));
        }
    }
    for h in 1..=2 {
        if let Some(end) = phrase(text, next, &format!("H{h}")) {
            return Some((
                end,
                PeriodTok::Half {
                    h,
                    year: Some(year),
                },
            ));
        }
    }
    if let Some(end) = phrase(text, next, "FY") {
        return Some((end, PeriodTok::Fy { year }));
    }
    let separator = text.get(next..)?.chars().next()?;
    if !matches!(separator, '-' | '–' | '—') {
        return None;
    }
    let right = spaces(text, next + separator.len_utf8());
    let digits = text
        .get(right..)?
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    let end = right + digits;
    if after_word(text, end) {
        return None;
    }
    let to = match digits {
        4 => text.get(right..end)?.parse::<u16>().ok()?,
        2 => (year / 100) * 100 + text.get(right..end)?.parse::<u16>().ok()?,
        _ => return None,
    };
    (to >= year).then_some((end, PeriodTok::YearRange { from: year, to }))
}

fn currency_at(text: &str, at: usize) -> Option<(usize, Currency)> {
    use Currency as C;
    [
        ("US$", C::Usd),
        ("USD", C::Usd),
        ("C$", C::Cad),
        ("CAD", C::Cad),
        ("A$", C::Aud),
        ("AUD", C::Aud),
        ("EUR", C::Eur),
        ("€", C::Eur),
        ("GBP", C::Gbp),
        ("£", C::Gbp),
        ("$", C::Dollar),
    ]
    .into_iter()
    .find_map(|(p, c)| prefix(text, at, p).map(|end| (end, c)))
}

fn unit(text: &str, at: usize) -> Option<(usize, UnitTok)> {
    use UnitTok as U;
    [
        ("US$/tonne", U::UsdPerTonne),
        ("USD/tonne", U::UsdPerTonne),
        ("US$/oz", U::UsdPerOz),
        ("USD/oz", U::UsdPerOz),
        ("US$/lb", U::UsdPerLb),
        ("USD/lb", U::UsdPerLb),
        ("$/oz", U::DollarPerOz),
        ("$/lb", U::DollarPerLb),
        ("grams per tonne", U::GramsPerTonne),
        ("g/t", U::GramsPerTonne),
        ("percent", U::Percent),
        ("per cent", U::Percent),
        ("%", U::Percent),
        ("koz", U::Koz),
        ("moz", U::Moz),
        ("kt", U::Kt),
        ("oz", U::Oz),
        ("ounces", U::Oz),
        ("lb", U::Lb),
        ("pounds", U::Lb),
        ("tonnes", U::Tonne),
        ("tonne", U::Tonne),
        ("USD", U::Usd),
    ]
    .into_iter()
    .find_map(|(p, u)| phrase(text, at, p).map(|end| (end, u)))
}

fn scale(text: &str, at: usize) -> Option<(usize, ScaleWord)> {
    use ScaleWord as S;
    [
        ("thousand", S::Thousand),
        ("million", S::Million),
        ("billion", S::Billion),
        ("k", S::Thousand),
        ("m", S::Million),
        ("bn", S::Billion),
        ("b", S::Billion),
    ]
    .into_iter()
    .find_map(|(p, s)| phrase(text, at, p).map(|end| (end, s)))
}

fn scaled(decimal: Dec, power: u32) -> Option<Dec> {
    if decimal.scale >= power {
        Some(Dec {
            mantissa: decimal.mantissa,
            scale: decimal.scale - power,
        })
    } else {
        Some(Dec {
            mantissa: decimal
                .mantissa
                .checked_mul(10_i128.checked_pow(power - decimal.scale)?)?,
            scale: 0,
        })
    }
}

fn exact_decimal(raw: &str, negative: bool) -> Option<(Dec, Dec)> {
    let mut parts = raw.split('.');
    let integer = parts.next()?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some() || fraction.contains(',') {
        return None;
    }
    if integer.contains(',') {
        let groups: Vec<_> = integer.split(',').collect();
        if groups.first().is_none_or(|s| s.is_empty() || s.len() > 3)
            || groups.iter().skip(1).any(|g| g.len() != 3)
        {
            return None;
        }
    }
    let scale = u32::try_from(fraction.len()).ok()?;
    if scale > 38 {
        return None;
    }
    let mut mantissa = 0_u128;
    let mut n = 0;
    for digit in integer
        .bytes()
        .filter(|b| *b != b',')
        .chain(fraction.bytes())
    {
        if !digit.is_ascii_digit() {
            return None;
        }
        mantissa = mantissa
            .checked_mul(10)?
            .checked_add(u128::from(digit - b'0'))?;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let mantissa = if negative && mantissa == (i128::MAX as u128) + 1 {
        i128::MIN
    } else {
        let positive = i128::try_from(mantissa).ok()?;
        if negative { -positive } else { positive }
    };
    Some((Dec { mantissa, scale }, Dec { mantissa: 1, scale }))
}

fn unsupported(text: &str, start: usize, end: usize, reason: &'static str) -> Token {
    Token::UnsupportedNumber {
        span: (start, end),
        raw: text[start..end].to_string(),
        reason,
    }
}

/// Parse one amount. `inherited` only supplies an explicitly printed range's common currency.
fn amount(text: &str, start: usize, inherited: Option<Currency>) -> Option<Token> {
    let mut at = start;
    let mut hedge = None;
    for (p, h) in [
        ("approximately", Hedge::Approximately),
        ("close to", Hedge::CloseTo),
        ("about", Hedge::About),
    ] {
        if let Some(end) = phrase(text, at, p) {
            hedge = Some(h);
            at = spaces(text, end);
            break;
        }
    }
    let paren_negative = text.get(at..)?.starts_with('(');
    if paren_negative {
        at = spaces(text, at + 1);
    }
    let mut negative = paren_negative;
    let mut explicit_sign = false;
    if let Some(c) = text
        .get(at..)?
        .chars()
        .next()
        .filter(|c| matches!(c, '-' | '−' | '+'))
    {
        negative = c != '+';
        explicit_sign = true;
        at = spaces(text, at + c.len_utf8());
    }
    let mut currency = inherited;
    if let Some((end, c)) = currency_at(text, at) {
        currency = Some(c);
        at = spaces(text, end);
    }
    if !explicit_sign
        && let Some(c) = text
            .get(at..)?
            .chars()
            .next()
            .filter(|c| matches!(c, '-' | '−' | '+'))
    {
        negative = c != '+';
        explicit_sign = true;
        at = spaces(text, at + c.len_utf8());
    }
    let number_start = at;
    while let Some(&b) = text.as_bytes().get(at) {
        if b.is_ascii_digit() {
            at += 1;
            continue;
        }
        if matches!(b, b',' | b'.')
            && text
                .as_bytes()
                .get(at + 1)
                .is_some_and(|next| next.is_ascii_digit() || *next == b',' || *next == b'.')
        {
            at += 1;
            continue;
        }
        break;
    }
    if number_start == at || !text[number_start..at].bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    let parsed = exact_decimal(&text[number_start..at], negative);
    let mut scale_word = None;
    let mut unit_kind = None;
    // Unit names such as `koz` take precedence over the single-letter scaling suffix `k`.
    if let Some((end, u)) = unit(text, spaces(text, at)) {
        unit_kind = Some(u);
        at = end;
    } else if let Some((end, s)) = scale(text, spaces(text, at)) {
        scale_word = Some(s);
        at = end;
        if let Some((end, u)) = unit(text, spaces(text, at)) {
            unit_kind = Some(u);
            at = end;
        }
    }
    let suffix = spaces(text, at);
    if unit_kind.is_none() {
        for (p, usd, dollar) in [
            ("/oz", UnitTok::UsdPerOz, Some(UnitTok::DollarPerOz)),
            ("/lb", UnitTok::UsdPerLb, Some(UnitTok::DollarPerLb)),
            ("/tonne", UnitTok::UsdPerTonne, None),
        ] {
            if let Some(end) = phrase(text, suffix, p) {
                let matched = match currency {
                    Some(Currency::Usd) => Some(usd),
                    Some(Currency::Dollar) => dollar,
                    _ => None,
                };
                if let Some(u) = matched {
                    unit_kind = Some(u);
                    at = end;
                }
                break;
            }
        }
    }
    if after_word(text, at) {
        while let Some(c) = text
            .get(at..)
            .and_then(|s| s.chars().next())
            .filter(|c| word(*c) || matches!(c, '.' | '+' | '-'))
        {
            at += c.len_utf8();
        }
        return Some(unsupported(
            text,
            start,
            at,
            "unsupported attached numeric notation",
        ));
    }
    if paren_negative {
        let end = spaces(text, at);
        if !text.get(end..).is_some_and(|s| s.starts_with(')')) {
            return Some(unsupported(
                text,
                start,
                at,
                "unclosed accounting parentheses",
            ));
        }
        at = end + 1;
        if explicit_sign {
            return Some(unsupported(
                text,
                start,
                at,
                "accounting parentheses with an explicit sign",
            ));
        }
    }
    let Some((mut value, mut step)) = parsed else {
        return Some(unsupported(
            text,
            start,
            at,
            "malformed or out-of-range exact decimal",
        ));
    };
    if let Some(s) = scale_word {
        let Some((v, d)) = scaled(value, s.power()).zip(scaled(step, s.power())) else {
            return Some(unsupported(
                text,
                start,
                at,
                "scaled decimal exceeds exact range",
            ));
        };
        value = v;
        step = d;
    }
    Some(Token::Number(NumberToken {
        span: (start, at),
        raw: text[start..at].to_string(),
        value,
        step,
        scale_word,
        currency,
        unit: unit_kind,
        percent: unit_kind == Some(UnitTok::Percent),
        paren_negative,
        hedge,
        range_with: None,
        range_step: None,
    }))
}

fn number(text: &str, at: usize) -> Option<Token> {
    let token = amount(text, at, None)?;
    let Token::Number(mut left) = token else {
        return Some(token);
    };
    let next = spaces(text, left.span.1);
    let end = if let Some(end) = phrase(text, next, "to") {
        end
    } else if let Some(c) = text
        .get(next..)
        .and_then(|s| s.chars().next())
        .filter(|c| matches!(c, '-' | '–' | '—'))
    {
        next + c.len_utf8()
    } else {
        return Some(Token::Number(left));
    };
    let right_start = spaces(text, end);
    let Some(Token::Number(right)) = amount(text, right_start, left.currency) else {
        return Some(Token::Number(left));
    };
    if left
        .currency
        .zip(right.currency)
        .is_some_and(|(a, b)| a != b)
        || left.unit.zip(right.unit).is_some_and(|(a, b)| a != b)
        || right.hedge.is_some()
    {
        return Some(Token::Number(left));
    }
    if left.scale_word.is_none()
        && let Some(s) = right.scale_word
    {
        let Some((v, d)) = scaled(left.value, s.power()).zip(scaled(left.step, s.power())) else {
            return Some(unsupported(
                text,
                at,
                right.span.1,
                "scaled range exceeds exact range",
            ));
        };
        left.value = v;
        left.step = d;
        left.scale_word = Some(s);
    }
    left.currency = left.currency.or(right.currency);
    left.unit = left.unit.or(right.unit);
    left.percent = left.unit == Some(UnitTok::Percent);
    left.range_with = Some(right.value);
    left.range_step = Some(right.step);
    left.span.1 = right.span.1;
    left.raw = text[at..right.span.1].to_string();
    Some(Token::Number(left))
}

/// Finite supported grammar only. Unrecognised prose is left untouched for the caller's supported-predicate
/// check. Period and basis metadata consume their own spans so they cannot become extra monetary amounts.
#[must_use]
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut result = Vec::new();
    let mut at = 0;
    while let Some(c) = text.get(at..).and_then(|s| s.chars().next()) {
        if start_boundary(text, at) {
            let token = if let Some((end, kind)) = basis(text, at) {
                Some(Token::Basis {
                    span: (at, end),
                    kind,
                })
            } else if let Some((end, kind)) = period(text, at) {
                Some(Token::Period {
                    span: (at, end),
                    kind,
                })
            } else if matches!(c, '–' | '—') {
                amount(text, spaces(text, at + c.len_utf8()), None)
                    .map(|t| unsupported(text, at, t.span().1, "ambiguous standalone dash sign"))
            } else {
                number(text, at).or_else(|| {
                    unit(text, at).map(|(end, kind)| Token::Unit {
                        span: (at, end),
                        kind,
                    })
                })
            };
            if let Some(token) = token {
                at = token.span().1;
                result.push(token);
                continue;
            }
        }
        at += c.len_utf8();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(mantissa: i128, scale: u32) -> Dec {
        Dec { mantissa, scale }
    }
    fn one(text: &str) -> NumberToken {
        let tokens = tokenize(text);
        assert_eq!(tokens.len(), 1, "{text}: {tokens:?}");
        let Token::Number(number) = &tokens[0] else {
            panic!("expected number: {tokens:?}");
        };
        assert_eq!(&text[number.span.0..number.span.1], number.raw);
        number.clone()
    }

    #[test]
    fn printed_digits_define_exact_value_and_step() {
        for (text, value, step) in [
            ("1,637", dec(1637, 0), dec(1, 0)),
            ("1.64k", dec(1640, 0), dec(10, 0)),
            ("$1.6 thousand", dec(1600, 0), dec(100, 0)),
            ("190%", dec(190, 0), dec(1, 0)),
            ("1,640", dec(1640, 0), dec(1, 0)),
            ("1.60", dec(160, 2), dec(1, 2)),
            (".50", dec(50, 2), dec(1, 2)),
            ("2.50 million", dec(2500000, 0), dec(10000, 0)),
        ] {
            let number = one(text);
            assert_eq!(number.value, value, "{text}");
            assert_eq!(number.step, step, "{text}");
        }
        for text in ["US$1,637", "$1.637k", "1,637"] {
            let number = one(text);
            assert_eq!((number.value, number.step), (dec(1637, 0), dec(1, 0)));
        }
        assert!(one("190%").percent);
    }

    #[test]
    fn accounting_signs_units_and_currency_are_preserved() {
        let n = one("(708)");
        assert_eq!(n.value, dec(-708, 0));
        assert!(n.paren_negative);
        assert_eq!(one("−708").value, dec(-708, 0));
        assert_eq!(one("($708)").value, dec(-708, 0));
        assert_eq!(one("-US$708").currency, Some(Currency::Usd));
        let usd = one("US$1,637/oz");
        assert_eq!(usd.unit, Some(UnitTok::UsdPerOz));
        let unspecified = one("$1,637/oz");
        assert_eq!(unspecified.currency, Some(Currency::Dollar));
        assert_eq!(unspecified.unit, Some(UnitTok::DollarPerOz));
        assert_eq!(one("US$1.25/lb").unit, Some(UnitTok::UsdPerLb));
        assert_eq!(one("$1.25/lb").unit, Some(UnitTok::DollarPerLb));
        assert_eq!(one("C1 cash costs are US$1.25/lb").value, dec(125, 2));
        let n = one("1.6 koz");
        assert_eq!(
            (n.value, n.step, n.unit),
            (dec(16, 1), dec(1, 1), Some(UnitTok::Koz))
        );
        assert_eq!(n.scale_word, None, "a unit is not a scale-word conversion");
        assert_eq!(one("1.6 Moz").unit, Some(UnitTok::Moz));
        assert_eq!(one("€1.6 million").currency, Some(Currency::Eur));
        for text in ["2.50 g/t", "2.50g/t", "2.50 grams per tonne"] {
            let n = one(text);
            assert_eq!(
                (n.value, n.step, n.unit),
                (dec(250, 2), dec(1, 2), Some(UnitTok::GramsPerTonne))
            );
            assert!(!n.percent);
            assert_eq!(
                n.scale_word, None,
                "a grade unit does not rescale its value"
            );
        }
    }

    #[test]
    fn ranges_keep_both_endpoint_precisions() {
        let n = one("200 - 230");
        assert_eq!((n.value, n.range_with), (dec(200, 0), Some(dec(230, 0))));
        assert_eq!((n.step, n.range_step), (dec(1, 0), Some(dec(1, 0))));
        let n = one("3.40 to 3.75");
        assert_eq!((n.value, n.range_with), (dec(340, 2), Some(dec(375, 2))));
        assert_eq!((n.step, n.range_step), (dec(1, 2), Some(dec(1, 2))));
        let n = one("200–230 koz");
        assert_eq!(n.unit, Some(UnitTok::Koz));
        assert_eq!(n.range_with, Some(dec(230, 0)));
        let n = one("US$1.6–1.8 thousand/oz");
        assert_eq!((n.value, n.range_with), (dec(1600, 0), Some(dec(1800, 0))));
        assert_eq!(n.unit, Some(UnitTok::UsdPerOz));
    }

    #[test]
    fn hedges_are_qualifiers_without_an_invented_tolerance() {
        for (text, hedge) in [
            ("about 1.6 thousand", Hedge::About),
            ("close to 1.6 thousand", Hedge::CloseTo),
            ("approximately 1.6 thousand", Hedge::Approximately),
        ] {
            let n = one(text);
            assert_eq!(n.hedge, Some(hedge));
            assert_eq!((n.value, n.step), (dec(1600, 0), dec(100, 0)));
        }
        assert_eq!(one("about 1,640").step, dec(1, 0));
    }

    #[test]
    fn basis_words_are_lexical_and_never_default_ownership() {
        for (text, expected) in [
            ("equity share", BasisTok::Attributable),
            ("ATTRIBUTABLE", BasisTok::Attributable),
            ("100% basis", BasisTok::Hundred),
            ("100 % basis", BasisTok::Hundred),
            ("consolidated", BasisTok::Hundred),
            ("per ounce sold", BasisTok::PerOzSold),
            ("per oz produced", BasisTok::PerOzProduced),
            ("per pound sold", BasisTok::PerLbSold),
            ("per lb sold", BasisTok::PerLbSold),
            ("per pound produced", BasisTok::PerLbProduced),
            ("per lb produced", BasisTok::PerLbProduced),
            ("by-product", BasisTok::ByProduct),
            ("co-product", BasisTok::CoProduct),
            ("gold ounces", BasisTok::GoldOunces),
            ("gold equivalent", BasisTok::Geo),
            ("GEO", BasisTok::Geo),
            ("non-GAAP", BasisTok::NonGaap),
            ("API scorecard", BasisTok::Scorecard),
            ("compensation-adjusted", BasisTok::Scorecard),
            ("ownership basis unprinted", BasisTok::Unprinted),
            ("unprinted ownership", BasisTok::Unprinted),
        ] {
            assert_eq!(
                tokenize(text),
                vec![Token::Basis {
                    span: (0, text.len()),
                    kind: expected
                }]
            );
        }
        assert!(tokenize("unprinted geological attributablex").is_empty());
    }

    #[test]
    fn dated_periods_keep_absent_years_explicit_and_do_not_emit_amounts() {
        for (text, expected) in [
            (
                "Q2 2026",
                PeriodTok::Quarter {
                    q: 2,
                    year: Some(2026),
                },
            ),
            (
                "2026Q2",
                PeriodTok::Quarter {
                    q: 2,
                    year: Some(2026),
                },
            ),
            ("Q2", PeriodTok::Quarter { q: 2, year: None }),
            ("H1", PeriodTok::Half { h: 1, year: None }),
            ("first half", PeriodTok::Half { h: 1, year: None }),
            (
                "first half of 2026",
                PeriodTok::Half {
                    h: 1,
                    year: Some(2026),
                },
            ),
            (
                "2030H2",
                PeriodTok::Half {
                    h: 2,
                    year: Some(2030),
                },
            ),
            ("FY2025", PeriodTok::Fy { year: 2025 }),
            ("2025FY", PeriodTok::Fy { year: 2025 }),
            (
                "2023–25",
                PeriodTok::YearRange {
                    from: 2023,
                    to: 2025,
                },
            ),
            ("three-year", PeriodTok::ThreeYear),
        ] {
            assert_eq!(
                tokenize(text),
                vec![Token::Period {
                    span: (0, text.len()),
                    kind: expected
                }]
            );
        }
    }

    #[test]
    fn malformed_and_overflowing_numbers_stay_visible() {
        for text in [
            "1,23",
            "1,,000",
            "1.2.3",
            "1e3",
            "1e-3",
            "0xFF",
            "(-708)",
            "(708",
            "–708",
            "170141183460469231731687303715884105728",
            "0.000000000000000000000000000000000000001",
        ] {
            let tokens = tokenize(text);
            assert!(
                matches!(tokens.as_slice(), [Token::UnsupportedNumber { .. }]),
                "{text}: {tokens:?}"
            );
            assert_eq!(tokens[0].span(), (0, text.len()));
        }
        assert_eq!(
            one("170141183460469231731687303715884105727").value,
            dec(i128::MAX, 0)
        );
        assert_eq!(
            one("-170141183460469231731687303715884105728").value,
            dec(i128::MIN, 0)
        );
    }

    #[test]
    fn spans_are_original_bytes_and_claim_identifiers_are_not_amounts() {
        let text = "Écart: [US$1,637/oz](fmine:clm_1234abcd5678) in Q2 2026; 100% basis.";
        let tokens = tokenize(text);
        let numbers: Vec<_> = tokens
            .iter()
            .filter_map(|t| match t {
                Token::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].raw, "US$1,637/oz");
        for token in tokens {
            let (start, end) = token.span();
            assert!(text.is_char_boundary(start) && text.is_char_boundary(end));
            assert!(start < end && end <= text.len());
        }
        assert_eq!(
            tokenize("A draft has 7 unrelated numbers.")
                .iter()
                .filter(|t| matches!(t, Token::Number(_)))
                .count(),
            1
        );
    }

    #[test]
    fn arbitrary_utf8_is_total_and_spans_remain_valid() {
        use fmine_core::rand::{PhiloxStream, RngKey};
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 345,
            partition: 1,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        let alphabet = [
            'a', 'Q', 'H', 'F', 'Y', '0', '1', '2', '9', '.', ',', '-', '−', '–', '—', 'é', '界',
            '💎', '\n', '\u{a0}', '(', ')', '$', '€', '%', '/', ' ', 'k', '_',
        ];
        for _ in 0..500 {
            let text: String = (0..128)
                .map(|_| alphabet[(rng.next_u64() % alphabet.len() as u64) as usize])
                .collect();
            let mut prior_end = 0;
            for token in tokenize(&text) {
                let (start, end) = token.span();
                assert!(prior_end <= start && start < end && end <= text.len());
                assert!(text.is_char_boundary(start) && text.is_char_boundary(end));
                if let Token::Number(n) = token {
                    assert_eq!(n.raw, text[start..end]);
                }
                prior_end = end;
            }
        }
    }
}
