//! DESIGN (BD-345): one bounded equality grammar shared by cited checking and additive queries.
//! Known subject names and citation spans are supplied by the caller; this parser never creates a Claim.

use super::{BasisTok, PeriodTok, Span, Token, UnitTok};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metric {
    Aisc,
    Tcc,
    C1,
    CostOfSales,
    Production,
    CopperProduction,
    Sales,
    ProcessedGrade,
    Recovery,
}

impl Metric {
    #[must_use]
    pub const fn rate(self) -> bool {
        matches!(self, Self::Aisc | Self::Tcc | Self::C1 | Self::CostOfSales)
    }
}

pub struct Context {
    pub subjects: Vec<String>,
    pub metrics: Vec<Metric>,
    pub periods: Vec<PeriodTok>,
    pub bases: Vec<BasisTok>,
    pub units: Vec<UnitTok>,
    pub numbers: usize,
    pub supported: bool,
}

fn erase(text: &mut String, span: Span) {
    if text.get(span.0..span.1).is_some() {
        text.replace_range(span.0..span.1, &" ".repeat(span.1 - span.0));
    }
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Exact whole words with flexible whitespace; matching never consumes an unregistered qualifier.
fn erase_phrase(text: &mut String, phrase: &str) -> usize {
    if phrase.trim().is_empty() {
        return 0;
    }
    let mut count = 0;
    loop {
        let found = text.char_indices().find_map(|(start, _)| {
            if text[..start].chars().next_back().is_some_and(word) {
                return None;
            }
            let mut end = start;
            for (n, part) in phrase.split_whitespace().enumerate() {
                if n != 0 {
                    let before = end;
                    while let Some(c) = text[end..].chars().next().filter(|c| c.is_whitespace()) {
                        end += c.len_utf8();
                    }
                    if before == end {
                        return None;
                    }
                }
                if !text.get(end..)?.starts_with(part) {
                    return None;
                }
                end += part.len();
            }
            (!text[end..].chars().next().is_some_and(word)).then_some((start, end))
        });
        let Some(span) = found else {
            break;
        };
        erase(text, span);
        count += 1;
    }
    count
}

/// Recognizes the existing one-number equality language; unsupported residual prose stays unsupported.
#[must_use]
pub fn context(text: &str, tokens: &[Token], citations: &[Span], subjects: &[String]) -> Context {
    let mut rest = text.to_ascii_lowercase();
    let mut c = Context {
        subjects: Vec::new(),
        metrics: Vec::new(),
        periods: Vec::new(),
        bases: Vec::new(),
        units: Vec::new(),
        numbers: 0,
        supported: false,
    };
    for token in tokens {
        match token {
            Token::Basis { kind, .. } => c.bases.push(*kind),
            Token::Period { kind, .. } => c.periods.push(*kind),
            Token::Unit { kind, .. } => c.units.push(*kind),
            Token::Number(n) => {
                c.numbers += 1;
                if let Some(u) = n.unit {
                    c.units.push(u);
                }
            }
            Token::UnsupportedNumber { .. } => c.numbers += 1,
        }
        erase(&mut rest, token.span());
    }
    for &span in citations {
        erase(&mut rest, span);
    }
    for subject in subjects {
        for _ in 0..erase_phrase(&mut rest, subject) {
            c.subjects.push(subject.clone());
        }
    }
    for (label, m) in [
        ("all-in sustaining costs", Metric::Aisc),
        ("all in sustaining costs", Metric::Aisc),
        ("total cash costs", Metric::Tcc),
        ("copper c1 cash costs", Metric::C1),
        ("c1 cash costs", Metric::C1),
        ("copper c1", Metric::C1),
        ("c1", Metric::C1),
        ("cost of sales", Metric::CostOfSales),
        ("copper production", Metric::CopperProduction),
        ("copper produced", Metric::CopperProduction),
        ("gold production", Metric::Production),
        ("gold produced", Metric::Production),
        ("gold sales", Metric::Sales),
        ("gold sold", Metric::Sales),
        ("processed grade", Metric::ProcessedGrade),
        ("head grade", Metric::ProcessedGrade),
        ("recovery rate", Metric::Recovery),
        ("recovery", Metric::Recovery),
        ("production", Metric::Production),
        ("sales", Metric::Sales),
        ("aisc", Metric::Aisc),
        ("tcc", Metric::Tcc),
    ] {
        for _ in 0..erase_phrase(&mut rest, label) {
            c.metrics.push(m);
        }
    }
    for label in ["close to", "approximately", "about"] {
        erase_phrase(&mut rest, label);
    }
    let words: Vec<_> = rest
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ':' | '.' | ';'))
        .filter(|s| !s.is_empty())
        .collect();
    c.supported = words
        .iter()
        .filter(|&&w| matches!(w, "is" | "was" | "equals" | "="))
        .count()
        == 1
        && words.iter().all(|&w| {
            matches!(
                w,
                "is" | "was"
                    | "equals"
                    | "="
                    | "for"
                    | "in"
                    | "of"
                    | "the"
                    | "on"
                    | "an"
                    | "a"
                    | "basis"
                    | "reported"
            )
        });
    c
}
