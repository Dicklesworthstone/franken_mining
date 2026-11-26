//! Determinism: Exact
//!
//! Drill-release intercept phrase grammar and cited-mode fallback (`BD-350.S2`, `bd-gqu3.1`).
//!
//! Parses public drill-release phrases such as:
//! - `"Hole FM24-209D: 1.6 m @ 30.6 g/t within 16.3 m @ 47.07 g/t"`
//! - `"DDH-001: 18.5 m @ 6.40 g/t Au from 120.5 m to 139.0 m including 2.5 m @ 24.00 g/t Au"`
//! - `"RC-14: 32.8 ft @ 0.45 oz/st AuEq"`
//!
//! Unparseable prose never guesses or fabricates numbers; it returns
//! [`ParseOutcome::FallbackToCitedMode`] so a human can enter cited-mode rows that pass through
//! the exact same `fmine_explore::intercept` kernel.

use fmine_evidence::{Refusal, RefusalCode};
use fmine_explore::intercept::{GradeUnit, InclusionRelation, Intercept, LengthUnit, PrintedQty};

/// Outcome of attempting to parse a raw drill-release phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Phrase matched the deterministic intercept grammar without ambiguity.
    Parsed(Box<Intercept>),
    /// Grammar declined to guess; caller should supply cited-mode rows (`K.5` fallback).
    FallbackToCitedMode { raw_phrase: String, reason: String },
}

/// Explicitly constructed cited-mode fallback row when prose fails grammar parsing (`K.5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedInterceptRow<'a> {
    pub hole: &'a str,
    pub from_token: Option<&'a str>,
    pub to_token: Option<&'a str>,
    pub length_token: &'a str,
    pub length_unit: LengthUnit,
    pub grade_token: &'a str,
    pub grade_unit: GradeUnit,
    pub metal_symbol: Option<&'a str>,
    pub is_metal_equivalent: bool,
    pub true_width_stated: bool,
    pub relation: InclusionRelation,
    pub children: Vec<Intercept>,
}

/// Convert a cited-mode row directly into an [`Intercept`] for `fmine_explore::intercept`.
pub fn intercept_from_cited_row(row: CitedInterceptRow<'_>) -> Result<Intercept, Refusal> {
    let hole = row.hole.trim();
    if hole.is_empty() || hole.bytes().any(|b| b.is_ascii_control()) {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("invalid cited hole identifier {:?}", row.hole),
        ));
    }
    if let Some(sym) = row.metal_symbol
        && (sym.trim().is_empty() || sym.bytes().any(|b| b.is_ascii_control()))
    {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("invalid cited metal_symbol {sym:?}"),
        ));
    }
    if row.from_token.is_some() != row.to_token.is_some() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("hole {hole}: cited row requires both from_token and to_token or neither"),
        ));
    }
    let from = match row.from_token {
        Some(t) => Some(PrintedQty::parse_decimal(t)?),
        None => None,
    };
    let to = match row.to_token {
        Some(t) => Some(PrintedQty::parse_decimal(t)?),
        None => None,
    };
    Ok(Intercept {
        hole: hole.to_string(),
        from,
        to,
        length: PrintedQty::parse_decimal(row.length_token)?,
        length_unit: row.length_unit,
        grade: PrintedQty::parse_decimal(row.grade_token)?,
        grade_unit: row.grade_unit,
        metal_symbol: row.metal_symbol.map(|s| s.trim().to_string()),
        is_metal_equivalent: row.is_metal_equivalent,
        is_downhole_length: true,
        true_width_stated: row.true_width_stated,
        relation: row.relation,
        children: row.children,
    })
}

/// Parse one drill-release intercept phrase into a structured [`Intercept`] tree, or fall back
/// cleanly to [`ParseOutcome::FallbackToCitedMode`] without guessing.
#[must_use]
pub fn parse_intercept_phrase(raw_phrase: &str, default_hole: Option<&str>) -> ParseOutcome {
    let trimmed = raw_phrase.trim();
    if trimmed.is_empty() {
        return fallback(raw_phrase, "empty phrase");
    }

    let (hole, body) = match extract_hole_prefix(trimmed, default_hole) {
        Some(pair) => pair,
        None => return fallback(raw_phrase, "missing hole identifier"),
    };

    let true_width_stated = has_stated_true_width(body);

    // Check top-level `"<child> within <host>"` form (e.g. E656).
    if let Some((left, right)) = split_keyword_once(body, "within") {
        let child =
            match parse_single_segment(&hole, left, InclusionRelation::Within, true_width_stated) {
                Ok(c) => c,
                Err(reason) => return fallback(raw_phrase, &reason),
            };
        let mut host =
            match parse_single_segment(&hole, right, InclusionRelation::Host, true_width_stated) {
                Ok(h) => h,
                Err(reason) => return fallback(raw_phrase, &reason),
            };
        host.children.push(child);
        return ParseOutcome::Parsed(Box::new(host));
    }

    // Check `"<host> including <inc1> [and <inc2> ... | including <nested>]"` form.
    if let Some((host_str, rest)) = split_keyword_once(body, "including") {
        let mut host =
            match parse_single_segment(&hole, host_str, InclusionRelation::Host, true_width_stated)
            {
                Ok(h) => h,
                Err(reason) => return fallback(raw_phrase, &reason),
            };
        match parse_including_clause(&hole, rest, true_width_stated) {
            Ok(children) => {
                host.children = children;
                return ParseOutcome::Parsed(Box::new(host));
            }
            Err(reason) => return fallback(raw_phrase, &reason),
        }
    }

    // Single intercept without inclusions.
    match parse_single_segment(&hole, body, InclusionRelation::Host, true_width_stated) {
        Ok(host) => ParseOutcome::Parsed(Box::new(host)),
        Err(reason) => fallback(raw_phrase, &reason),
    }
}

fn parse_including_clause(
    hole: &str,
    clause: &str,
    true_width_stated: bool,
) -> Result<Vec<Intercept>, String> {
    // Nested `"<child> including <grandchild>"` chain; refuse ambiguous `"; including"`.
    if let Some((head, tail)) = split_keyword_once(clause, "including") {
        if head.trim_end().ends_with(';') {
            return Err("ambiguous '; including' clause; use cited mode".to_string());
        }
        let mut child =
            parse_single_segment(hole, head, InclusionRelation::Including, true_width_stated)?;
        child.children = parse_including_clause(hole, tail, true_width_stated)?;
        return Ok(vec![child]);
    }
    // Sibling inclusions separated by `" and "` or `";"` or `","`.
    let mut children = Vec::new();
    for piece in split_sibling_clauses(clause) {
        let p = piece.trim();
        if p.is_empty() {
            continue;
        }
        children.push(parse_single_segment(
            hole,
            p,
            InclusionRelation::Including,
            true_width_stated,
        )?);
    }
    if children.is_empty() {
        return Err("empty 'including' clause".to_string());
    }
    Ok(children)
}

fn split_sibling_clauses(clause: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for semi in clause.split(';') {
        for comma in semi.split(',') {
            let mut rem = comma;
            while let Some((l, r)) = split_keyword_once(rem, "and") {
                out.push(l);
                rem = r;
            }
            out.push(rem);
        }
    }
    out
}

fn parse_single_segment(
    hole: &str,
    segment: &str,
    relation: InclusionRelation,
    true_width_stated: bool,
) -> Result<Intercept, String> {
    // Strip trailing parenthetical notes like "(true width 15 m)" while refusing unbalanced parens
    // or parentheticals carrying intercept/grade content.
    let cleaned = strip_parenthetical_notes(segment)?;
    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| matches!(c, ',' | ';' | ':' | '(' | ')')))
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.len() < 3 {
        return Err(format!(
            "segment {segment:?} too short to form '<len> <unit> @ <grade> <unit>'"
        ));
    }

    // Locate `@` or `grading` or `at` separator between length and grade.
    let sep_idx = tokens
        .iter()
        .position(|t| {
            *t == "@" || t.eq_ignore_ascii_case("grading") || t.eq_ignore_ascii_case("at")
        })
        .ok_or_else(|| format!("missing '@' or 'grading' separator in {segment:?}"))?;
    if sep_idx < 1 || sep_idx + 1 >= tokens.len() {
        return Err(format!("incomplete length/grade pair in {segment:?}"));
    }

    // Parse length + unit right before `sep_idx` (handles both `"16.3 m"` and `"16.3m"`).
    let (len_tok, len_unit, len_start_idx) = parse_len_before(&tokens[..sep_idx])?;

    // Parse grade + unit right after `sep_idx` (handles `"1.5%"`, `"47.07g/t"`, and spaced forms).
    let (grade, grade_unit, mut cursor) = parse_grade_after(&tokens, sep_idx + 1)?;

    // Optional metal symbol right after grade unit (e.g. `Au`, `AuEq`, `Cu`, `CuEq`).
    let mut metal_symbol = None;
    let mut is_metal_equivalent = false;
    if cursor < tokens.len() && is_metal_token(tokens[cursor]) {
        let sym = tokens[cursor].trim_end_matches('.');
        is_metal_equivalent = sym.to_ascii_lowercase().ends_with("eq");
        metal_symbol = Some(sym.to_string());
        cursor += 1;
    }

    // Optional `"from X [m|ft] to Y [m|ft]"` strictly consuming remaining tokens before length or after grade.
    let pre_tokens = &tokens[..len_start_idx];
    let post_tokens = &tokens[cursor..];
    let pre_span = if pre_tokens.is_empty() {
        None
    } else {
        Some(parse_exact_from_to(pre_tokens, len_unit)?)
    };
    let post_span = if post_tokens.is_empty() {
        None
    } else {
        Some(parse_exact_from_to(post_tokens, len_unit)?)
    };
    let (from, to) = match (pre_span, post_span) {
        (Some(_), Some(_)) => return Err("duplicate 'from ... to ...' depth span".to_string()),
        (Some((f, t)), None) | (None, Some((f, t))) => (Some(f), Some(t)),
        (None, None) => (None, None),
    };

    Ok(Intercept {
        hole: hole.to_string(),
        from,
        to,
        length: len_tok,
        length_unit: len_unit,
        grade,
        grade_unit,
        metal_symbol,
        is_metal_equivalent,
        is_downhole_length: true,
        true_width_stated,
        relation,
        children: Vec::new(),
    })
}

fn parse_len_before(prefix: &[&str]) -> Result<(PrintedQty, LengthUnit, usize), String> {
    let last = *prefix.last().ok_or_else(|| "missing length".to_string())?;
    // Check glued form like "16.3m" or "32.8ft".
    if let Some((num_s, unit_s)) = split_glued_number_unit(last) {
        let qty = PrintedQty::parse_decimal(num_s)
            .map_err(|_| format!("invalid length token {num_s:?}"))?;
        let unit = parse_length_unit(unit_s)?;
        return Ok((qty, unit, prefix.len() - 1));
    }
    if prefix.len() < 2 {
        return Err("missing length number before length unit".to_string());
    }
    let unit = parse_length_unit(last)?;
    let num_s = prefix[prefix.len() - 2];
    let qty =
        PrintedQty::parse_decimal(num_s).map_err(|_| format!("invalid length token {num_s:?}"))?;
    Ok((qty, unit, prefix.len() - 2))
}

fn parse_length_unit(tok: &str) -> Result<LengthUnit, String> {
    match tok.trim_end_matches('.').to_ascii_lowercase().as_str() {
        "m" | "metre" | "metres" | "meter" | "meters" => Ok(LengthUnit::Metres),
        "ft" | "foot" | "feet" => Ok(LengthUnit::Feet),
        other => Err(format!("unrecognized length unit {other:?}")),
    }
}

fn parse_grade_after(
    tokens: &[&str],
    idx: usize,
) -> Result<(PrintedQty, GradeUnit, usize), String> {
    let tok = tokens[idx];
    if let Some((num_s, unit_s)) = split_glued_grade_unit(tok) {
        let grade = PrintedQty::parse_decimal(num_s)
            .map_err(|_| format!("invalid grade token {num_s:?}"))?;
        let (unit, _) = parse_grade_unit(&[unit_s], 0)?;
        return Ok((grade, unit, idx + 1));
    }
    if idx + 1 >= tokens.len() {
        return Err(format!("missing grade unit after {tok:?}"));
    }
    let grade =
        PrintedQty::parse_decimal(tok).map_err(|_| format!("invalid grade token {tok:?}"))?;
    let (unit, next_idx) = parse_grade_unit(tokens, idx + 1)?;
    Ok((grade, unit, next_idx))
}

fn split_glued_grade_unit(tok: &str) -> Option<(&str, &str)> {
    let split = tok.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '+' && c != '-')?;
    if split == 0 {
        return None;
    }
    Some((&tok[..split], &tok[split..]))
}

fn parse_grade_unit(tokens: &[&str], idx: usize) -> Result<(GradeUnit, usize), String> {
    let raw = tokens[idx].trim_end_matches('.');
    match raw.to_ascii_lowercase().as_str() {
        "g/t" | "gpt" | "g/mt" => Ok((GradeUnit::GramsPerTonne, idx + 1)),
        "oz/st" | "oz/t" | "oz/ton" | "opt" => Ok((GradeUnit::OzPerShortTon, idx + 1)),
        "%" | "pct" => Ok((GradeUnit::Percent, idx + 1)),
        "oz" if idx + 2 < tokens.len()
            && tokens[idx + 1] == "/"
            && matches!(
                tokens[idx + 2].to_ascii_lowercase().as_str(),
                "st" | "t" | "ton"
            ) =>
        {
            Ok((GradeUnit::OzPerShortTon, idx + 3))
        }
        other => Err(format!("unrecognized grade unit {other:?}")),
    }
}

fn parse_exact_from_to(
    tokens: &[&str],
    expected_unit: LengthUnit,
) -> Result<(PrintedQty, PrintedQty), String> {
    if !tokens
        .first()
        .is_some_and(|t| t.eq_ignore_ascii_case("from"))
    {
        return Err(format!("unconsumed tokens outside grammar: {tokens:?}"));
    }
    let rest = &tokens[1..];
    let Some(to_rel) = rest.iter().position(|t| t.eq_ignore_ascii_case("to")) else {
        return Err("found 'from' without matching 'to'".to_string());
    };
    let from_slice = &rest[..to_rel];
    let to_slice = &rest[to_rel + 1..];
    if from_slice.is_empty() || to_slice.is_empty() {
        return Err("incomplete 'from ... to ...' depth span".to_string());
    }
    let (from_qty, from_unit, from_used) = parse_depth_endpoint(from_slice, expected_unit)?;
    let (to_qty, to_unit, to_used) = parse_depth_endpoint(to_slice, expected_unit)?;
    if from_used != from_slice.len() || to_used != to_slice.len() {
        return Err(format!(
            "unconsumed tokens inside 'from ... to ...': {tokens:?}"
        ));
    }
    if from_unit != expected_unit || to_unit != expected_unit {
        return Err("mixed length units inside 'from ... to ...' depth span".to_string());
    }
    Ok((from_qty, to_qty))
}

fn parse_depth_endpoint(
    slice: &[&str],
    default_unit: LengthUnit,
) -> Result<(PrintedQty, LengthUnit, usize), String> {
    let first = slice[0];
    if let Some((num_s, unit_s)) = split_glued_number_unit(first) {
        let qty = PrintedQty::parse_decimal(num_s)
            .map_err(|_| format!("invalid depth token {num_s:?}"))?;
        let unit = parse_length_unit(unit_s)?;
        return Ok((qty, unit, 1));
    }
    let qty =
        PrintedQty::parse_decimal(first).map_err(|_| format!("invalid depth token {first:?}"))?;
    if slice.len() >= 2 {
        let unit = parse_length_unit(slice[1])?;
        Ok((qty, unit, 2))
    } else {
        Ok((qty, default_unit, 1))
    }
}

fn extract_hole_prefix<'a>(
    trimmed: &'a str,
    default_hole: Option<&str>,
) -> Option<(String, &'a str)> {
    // Form 1: "Hole FM24-209D: ..." or "FM24-209D: ..."
    if let Some((left, right)) = trimmed.split_once(':') {
        let left_trim = left.trim();
        let candidate = left_trim
            .strip_prefix("Hole ")
            .or_else(|| left_trim.strip_prefix("hole "))
            .or_else(|| left_trim.strip_prefix("HOLE "))
            .unwrap_or(left_trim)
            .trim();
        if is_hole_id(candidate) {
            return Some((candidate.to_string(), right.trim()));
        }
    }
    // Form 2: "FM24-209D intersected ..." or "Hole FM24-209D ..."
    let mut words = trimmed.split_whitespace();
    if let Some(w0) = words.next() {
        let (candidate, rest_start, explicit_hole_kw) = if w0.eq_ignore_ascii_case("hole") {
            let w1 = words.next()?;
            let after_kw = &trimmed[w0.len()..];
            let ws_len = after_kw.len() - after_kw.trim_start().len();
            let end = w0.len() + ws_len + w1.len();
            (w1.trim_end_matches(':'), &trimmed[end..], true)
        } else {
            (w0.trim_end_matches(':'), &trimmed[w0.len()..], false)
        };
        let looks_like_glued_length = !explicit_hole_kw
            && split_glued_number_unit(candidate).is_some_and(|(num, unit)| {
                PrintedQty::parse_decimal(num).is_ok() && parse_length_unit(unit).is_ok()
            });
        if !looks_like_glued_length && is_hole_id(candidate) {
            let body = rest_start.trim();
            let body = body
                .strip_prefix("intersected ")
                .or_else(|| body.strip_prefix("returned "))
                .unwrap_or(body)
                .trim();
            return Some((candidate.to_string(), body));
        }
    }
    default_hole.map(|h| (h.trim().to_string(), trimmed))
}

fn has_stated_true_width(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    if !lower.contains("true width") && !lower.contains("true-width") {
        return false;
    }
    for neg in [
        "unknown",
        "not determined",
        "not yet",
        "not known",
        "not reported",
        "not stated",
        "cannot be",
        "unconfirmed",
        "no true width",
        "no true-width",
        "n/a",
        "unavailable",
    ] {
        if lower.contains(neg) {
            return false;
        }
    }
    // Require a quantitative figure inside the true-width statement (e.g. "true width 15 m" or "75%").
    let pos = lower
        .find("true width")
        .or_else(|| lower.find("true-width"))
        .unwrap_or(0);
    let window_start = pos.saturating_sub(24);
    let window_end = (pos + 40).min(lower.len());
    lower
        .get(window_start..window_end)
        .is_some_and(|w| w.bytes().any(|b| b.is_ascii_digit()))
}

fn is_hole_id(s: &str) -> bool {
    !s.is_empty()
        && !s.contains(char::is_whitespace)
        && s.bytes().any(|b| b.is_ascii_alphabetic())
        && s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn is_metal_token(tok: &str) -> bool {
    let clean = tok.trim_end_matches('.');
    !clean.is_empty()
        && clean.len() <= 6
        && clean.bytes().all(|b| b.is_ascii_alphabetic())
        && !clean.eq_ignore_ascii_case("from")
        && !clean.eq_ignore_ascii_case("to")
        && !clean.eq_ignore_ascii_case("and")
        && !clean.eq_ignore_ascii_case("including")
        && !clean.eq_ignore_ascii_case("within")
}

fn split_glued_number_unit(tok: &str) -> Option<(&str, &str)> {
    let split = tok.find(|c: char| c.is_ascii_alphabetic())?;
    if split == 0 {
        return None;
    }
    Some((&tok[..split], &tok[split..]))
}

fn split_keyword_once<'a>(s: &'a str, kw: &str) -> Option<(&'a str, &'a str)> {
    let lower = s.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let kw_bytes = kw.as_bytes();
    if bytes.len() < kw_bytes.len() {
        return None;
    }
    for i in 0..=bytes.len() - kw_bytes.len() {
        if &bytes[i..i + kw_bytes.len()] == kw_bytes {
            let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let right_end = i + kw_bytes.len();
            let right_ok = right_end == bytes.len() || !bytes[right_end].is_ascii_alphanumeric();
            if left_ok && right_ok {
                return Some((s[..i].trim(), s[right_end..].trim()));
            }
        }
    }
    None
}

fn strip_parenthetical_notes(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut paren_buf = String::new();
    let mut depth = 0usize;
    for ch in s.chars() {
        match ch {
            '(' => {
                if depth == 0 {
                    paren_buf.clear();
                } else {
                    paren_buf.push('(');
                }
                depth += 1;
            }
            ')' => {
                if depth == 0 {
                    return Err("unbalanced closing ')' in intercept phrase".to_string());
                }
                depth -= 1;
                if depth == 0 {
                    let lower = paren_buf.to_ascii_lowercase();
                    for forbidden in [
                        "@", "g/t", "gpt", "oz/st", "oz/t", "oz/ton", "includ", "within", "grading",
                    ] {
                        if lower.contains(forbidden) {
                            return Err(format!(
                                "parenthetical note contains intercept/grade data ({paren_buf:?}); use cited mode"
                            ));
                        }
                    }
                } else {
                    paren_buf.push(')');
                }
            }
            c if depth == 0 => out.push(c),
            c => paren_buf.push(c),
        }
    }
    if depth != 0 {
        return Err("unbalanced opening '(' in intercept phrase".to_string());
    }
    Ok(out)
}

fn fallback(raw_phrase: &str, reason: &str) -> ParseOutcome {
    ParseOutcome::FallbackToCitedMode {
        raw_phrase: raw_phrase.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_explore::intercept::{
        DisclosureFlag, ReleaseDisclosureContext, audit_drill_release, remainder_grade,
    };
    use fmine_units::ExactQty;

    #[test]
    fn test_parse_e656_within_phrase_and_reproduce_remainder() {
        let phrase = "Hole FM24-209D: 1.6 m @ 30.6 g/t within 16.3 m @ 47.07 g/t";
        let ParseOutcome::Parsed(intercept) = parse_intercept_phrase(phrase, None) else {
            panic!("expected E656 phrase to parse");
        };
        assert_eq!(intercept.hole, "FM24-209D");
        assert_eq!(intercept.children.len(), 1);
        assert_eq!(intercept.children[0].relation, InclusionRelation::Within);

        let rem = remainder_grade(&intercept).expect("E656 remainder");
        assert_eq!(
            rem.remainder_grade_point,
            ExactQty::new(718_281, 14_700).unwrap()
        );
    }

    #[test]
    fn test_parse_including_from_to_and_imperial_units() {
        let phrase = "DDH-001: 18.5 m @ 6.40 g/t Au from 120.5 m to 139.0 m including 2.5 m @ 24.00 g/t Au from 124.0 m to 126.5 m and 1.0 m @ 15.20 g/t Au from 130.0 m to 131.0 m (true width 15 m)";
        let ParseOutcome::Parsed(intercept) = parse_intercept_phrase(phrase, None) else {
            panic!("expected multi-inclusion phrase to parse");
        };
        assert_eq!(intercept.hole, "DDH-001");
        assert!(intercept.true_width_stated);
        assert_eq!(intercept.children.len(), 2);
        assert!(intercept.from.is_some() && intercept.to.is_some());
        let rem = remainder_grade(&intercept).expect("valid non-overlapping remainder");
        assert_eq!(rem.remainder_length_m, ExactQty::from_integer(15));

        let imp = "RC-14: 32.8 ft @ 0.45 oz/st AuEq";
        let ParseOutcome::Parsed(imp_ic) = parse_intercept_phrase(imp, None) else {
            panic!("expected imperial AuEq phrase to parse");
        };
        assert_eq!(imp_ic.length_unit, LengthUnit::Feet);
        assert_eq!(imp_ic.grade_unit, GradeUnit::OzPerShortTon);
        assert!(imp_ic.is_metal_equivalent);
        assert!(!imp_ic.true_width_stated);
    }

    #[test]
    fn test_unparsed_phrase_falls_back_to_cited_mode() {
        let messy =
            "Hole SYN-99 intersected visible gold across broad quartz veining; assays pending.";
        let outcome = parse_intercept_phrase(messy, None);
        assert!(
            matches!(outcome, ParseOutcome::FallbackToCitedMode { .. }),
            "unparseable prose must fall back to cited mode without guessing"
        );

        // Unconsumed trailing prose must also fall back rather than silently dropping a second interval.
        let ambiguous = "Hole SYN-99: 10.0 m @ 2.0 g/t Au and 5.0 m @ 3.0 g/t Au";
        assert!(
            matches!(
                parse_intercept_phrase(ambiguous, None),
                ParseOutcome::FallbackToCitedMode { .. }
            ),
            "compound phrase without 'including'/'within' must fall back instead of dropping tokens"
        );

        // Cited-mode fallback feeds the exact same audit kernel.
        let cited = intercept_from_cited_row(CitedInterceptRow {
            hole: "SYN-99",
            from_token: Some("100.0"),
            to_token: Some("110.0"),
            length_token: "10.0",
            length_unit: LengthUnit::Metres,
            grade_token: "2.0",
            grade_unit: GradeUnit::GramsPerTonne,
            metal_symbol: Some("Au"),
            is_metal_equivalent: false,
            true_width_stated: true,
            relation: InclusionRelation::Host,
            children: vec![
                intercept_from_cited_row(CitedInterceptRow {
                    hole: "SYN-99",
                    from_token: Some("102.0"),
                    to_token: Some("103.0"),
                    length_token: "1.0",
                    length_unit: LengthUnit::Metres,
                    grade_token: "17.3",
                    grade_unit: GradeUnit::GramsPerTonne,
                    metal_symbol: Some("Au"),
                    is_metal_equivalent: false,
                    true_width_stated: true,
                    relation: InclusionRelation::Including,
                    children: vec![],
                })
                .unwrap(),
            ],
        })
        .unwrap();
        let ctx = ReleaseDisclosureContext::complete(
            ExactQty::new(1, 2).unwrap(),
            GradeUnit::GramsPerTonne,
        );
        let audit = audit_drill_release(&[cited], &ctx).unwrap();
        assert!(
            audit.intervals[0]
                .flags
                .contains(&DisclosureFlag::RemainderBelowCutoff {
                    hole: "SYN-99".to_string()
                })
        );
    }

    #[test]
    fn test_fresh_eyes_grammar_edge_cases() {
        // 1. Negated true-width disclaimers must NOT suppress NoTrueWidth.
        let unknown_tw = "DDH-010: 12.0 m @ 4.0 g/t Au (true width unknown)";
        let ParseOutcome::Parsed(ic) = parse_intercept_phrase(unknown_tw, None) else {
            panic!("expected unknown true width phrase to parse");
        };
        assert!(!ic.true_width_stated);

        // 2. Glued length token "16m" must not be misparsed as a hole ID when default_hole is supplied.
        let glued = "16m @ 47.07g/t";
        let ParseOutcome::Parsed(glued_ic) = parse_intercept_phrase(glued, Some("FM24-209D"))
        else {
            panic!("expected glued '16m @ 47.07g/t' to parse with default_hole");
        };
        assert_eq!(glued_ic.hole, "FM24-209D");
        assert_eq!(glued_ic.length.value, ExactQty::from_integer(16));
        assert_eq!(glued_ic.grade.value, ExactQty::new(4707, 100).unwrap());

        // 3. Glued copper percentage "1.5% Cu" parses cleanly.
        let cu = "DDH-011: 12.0 m @ 1.5% Cu";
        let ParseOutcome::Parsed(cu_ic) = parse_intercept_phrase(cu, None) else {
            panic!("expected glued '1.5% Cu' to parse");
        };
        assert_eq!(cu_ic.grade_unit, GradeUnit::Percent);
        assert_eq!(cu_ic.metal_symbol.as_deref(), Some("Cu"));

        // 4. Parenthetical containing an inclusion, unbalanced parens, and "; including" all fall back.
        for tricky in [
            "DDH-012: 18.5 m @ 6.40 g/t Au (includes 2.5 m @ 24.0 g/t)",
            "DDH-012: 18.5 m @ 6.40 g/t Au (true width 12 m",
            "DDH-012: 20.0 m @ 4.0 g/t including 5.0 m @ 10.0 g/t; including 3.0 m @ 8.0 g/t",
        ] {
            assert!(
                matches!(
                    parse_intercept_phrase(tricky, None),
                    ParseOutcome::FallbackToCitedMode { .. }
                ),
                "expected fallback on {tricky:?}"
            );
        }
    }
}
