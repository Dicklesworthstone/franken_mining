//! Cell number formats (bd-jg43.1): the `numFmts` and `cellXfs` of a workbook's styles part, and each cell's
//! format resolved from its `s` attribute by ECMA-376 Part 1 (§18.3.1.4 `c`, §18.8.30 `numFmt`).
//!
//! Resolution never guesses. A cell without `s` uses xf 0. An `s` past the end of `cellXfs` is
//! [`NumberFormat::BadStyleIndex`], and a `numFmtId` that is neither listed in `numFmts` nor built in is
//! [`NumberFormat::Unknown`]; neither is clamped or defaulted to General. A workbook with no styles part formats
//! every cell as built-in 0 (General).

use std::collections::BTreeMap;

use crate::IoError;
use crate::xlsx::{decode_xml_entities, extract_attr_exact, iter_xml_start_tags};

/// The built-in formats ECMA-376 Part 1 §18.8.30 defines, by `numFmtId`. Ids 5–8, 23–36 and 41–44 are
/// locale-dependent and not defined there; a workbook that uses them must list them in `numFmts`.
pub const BUILTIN_FORMATS: [(u32, &str); 28] = [
    (0, "General"),
    (1, "0"),
    (2, "0.00"),
    (3, "#,##0"),
    (4, "#,##0.00"),
    (9, "0%"),
    (10, "0.00%"),
    (11, "0.00E+00"),
    (12, "# ?/?"),
    (13, "# ??/??"),
    (14, "mm-dd-yy"),
    (15, "d-mmm-yy"),
    (16, "d-mmm"),
    (17, "mmm-yy"),
    (18, "h:mm AM/PM"),
    (19, "h:mm:ss AM/PM"),
    (20, "h:mm"),
    (21, "h:mm:ss"),
    (22, "m/d/yy h:mm"),
    (37, "#,##0 ;(#,##0)"),
    (38, "#,##0 ;[Red](#,##0)"),
    (39, "#,##0.00;(#,##0.00)"),
    (40, "#,##0.00;[Red](#,##0.00)"),
    (45, "mm:ss"),
    (46, "[h]:mm:ss"),
    (47, "mmss.0"),
    (48, "##0.0E+0"),
    (49, "@"),
];

/// The built-in format code for `id`, if ECMA-376 defines one.
#[must_use]
pub fn builtin_code(id: u32) -> Option<&'static str> {
    BUILTIN_FORMATS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, code)| *code)
}

/// A cell's number format as resolved from its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberFormat {
    /// A built-in id from [`BUILTIN_FORMATS`] that the workbook does not redefine.
    Builtin(u32),
    /// A format listed in the workbook's `numFmts`, with its code decoded from XML.
    Custom { id: u32, code: String },
    /// A `numFmtId` neither listed nor built in.
    Unknown(u32),
    /// An `s` attribute past the end of `cellXfs`.
    BadStyleIndex(u32),
}

impl NumberFormat {
    /// The format code, or a description naming the id or index that resolves to none. This is the key of
    /// `Workbook::format_census` and of `scripts/oracle/formats.py`.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Builtin(id) => builtin_code(*id)
                .map_or_else(|| format!("unknown numFmtId {id}"), ToString::to_string),
            Self::Custom { code, .. } => code.clone(),
            Self::Unknown(id) => format!("unknown numFmtId {id}"),
            Self::BadStyleIndex(s) => format!("bad style index {s}"),
        }
    }
}

/// A workbook's styles part, reduced to what number formats need.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Styles {
    /// `numFmts`: id to format code (entities decoded).
    pub num_fmts: BTreeMap<u32, String>,
    /// `cellXfs`, in order: each xf's `numFmtId` (absent means 0).
    pub cell_xf_num_fmt: Vec<u32>,
}

impl Styles {
    /// The number format of a cell whose `s` attribute is `style` (absent: xf 0).
    #[must_use]
    pub fn format_of(&self, style: Option<u32>) -> NumberFormat {
        let s = style.unwrap_or(0);
        let Some(&id) = usize::try_from(s)
            .ok()
            .and_then(|i| self.cell_xf_num_fmt.get(i))
        else {
            return NumberFormat::BadStyleIndex(s);
        };
        if let Some(code) = self.num_fmts.get(&id) {
            NumberFormat::Custom {
                id,
                code: code.clone(),
            }
        } else if builtin_code(id).is_some() {
            NumberFormat::Builtin(id)
        } else {
            NumberFormat::Unknown(id)
        }
    }
}

/// The body of the first `<name ...>...</name>` element, or `None` if it is absent or self-closing.
fn section<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = iter_xml_start_tags(xml, name).into_iter().next()?;
    if open.ends_with("/>") {
        return None;
    }
    let start = xml.find(open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(&xml[start..end])
}

fn id_attr(tag: &str, name: &str) -> Result<Option<u32>, IoError> {
    extract_attr_exact(tag, name)
        .map(|v| {
            v.trim().parse::<u32>().map_err(|_| {
                IoError::Xml(format!("styles: {name} {v:?} is not an unsigned integer"))
            })
        })
        .transpose()
}

/// Parses a styles part: every `<numFmt>` of `<numFmts>` and the `numFmtId` of every `<xf>` of `<cellXfs>`, in
/// order. `<cellStyleXfs>` is not read. A `numFmt` without a valid id or a code, or an `xf` whose `numFmtId`
/// is not an unsigned integer, is an error.
pub fn parse_styles(xml: &str) -> Result<Styles, IoError> {
    let mut styles = Styles::default();
    if let Some(body) = section(xml, "numFmts") {
        for tag in iter_xml_start_tags(body, "numFmt") {
            let id = id_attr(tag, "numFmtId")?
                .ok_or_else(|| IoError::Xml("styles: numFmt without numFmtId".to_string()))?;
            let code = extract_attr_exact(tag, "formatCode")
                .ok_or_else(|| IoError::Xml(format!("styles: numFmt {id} without formatCode")))?;
            styles.num_fmts.insert(id, decode_xml_entities(&code));
        }
    }
    if let Some(body) = section(xml, "cellXfs") {
        for tag in iter_xml_start_tags(body, "xf") {
            styles
                .cell_xf_num_fmt
                .push(id_attr(tag, "numFmtId")?.unwrap_or(0));
        }
    }
    Ok(styles)
}

/// The styles part named by the workbook relationships (`Type` ending in `/styles`), else `xl/styles.xml`.
#[must_use]
pub fn styles_part_name(rels_xml: &str) -> Option<String> {
    iter_xml_start_tags(rels_xml, "Relationship")
        .into_iter()
        .filter(|tag| extract_attr_exact(tag, "Type").is_some_and(|t| t.ends_with("/styles")))
        .find_map(|tag| extract_attr_exact(tag, "Target"))
        .map(|target| {
            let t = target.trim_start_matches('/');
            if t.starts_with("xl/") {
                t.to_string()
            } else {
                format!("xl/{t}")
            }
        })
}

/// Whether a stored decimal (optional sign, digits with an optional point, optional exponent) is not an integer,
/// decided exactly on the text. `None` for text that is not such a decimal.
#[must_use]
pub fn decimal_is_non_integer(text: &str) -> Option<bool> {
    let t = text.trim();
    let t = t.strip_prefix(['+', '-']).unwrap_or(t);
    let (mantissa, exponent) = match t.find(['e', 'E']) {
        Some(i) => {
            let e = &t[i + 1..];
            let digits = e.strip_prefix(['+', '-']).unwrap_or(e);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            (
                &t[..i],
                e.parse::<i64>().unwrap_or(if e.starts_with('-') {
                    i64::MIN / 2
                } else {
                    i64::MAX / 2
                }),
            )
        }
        None => (t, 0),
    };
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if (int_part.is_empty() && frac_part.is_empty())
        || !int_part
            .bytes()
            .chain(frac_part.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    // The digits int_part ++ frac_part, with the point after int_part, moved `exponent` places right.
    let point = i64::try_from(int_part.len())
        .unwrap_or(i64::MAX)
        .saturating_add(exponent);
    let first_fractional = usize::try_from(point.max(0)).unwrap_or(usize::MAX);
    Some(
        int_part
            .bytes()
            .chain(frac_part.bytes())
            .skip(first_fractional)
            .any(|b| b != b'0'),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLES: &str = r#"<?xml version="1.0"?><styleSheet xmlns="x"><numFmts count="2"><numFmt numFmtId="164" formatCode="_(* #,##0_);_(* \(#,##0\);_(* &quot;-&quot;??_);_(@_)"/><numFmt numFmtId="3" formatCode="0.0"/></numFmts><cellStyleXfs count="1"><xf numFmtId="49"/></cellStyleXfs><cellXfs count="4"><xf numFmtId="0" fontId="0"/><xf fontId="1" numFmtId="164" applyNumberFormat="1"/><xf fontId="2"/><xf numFmtId="200"/></cellXfs></styleSheet>"#;

    #[test]
    fn test_parse_styles_numfmts_and_cellxfs() {
        let s = parse_styles(STYLES).expect("styles");
        assert_eq!(s.num_fmts.len(), 2);
        assert_eq!(
            s.cell_xf_num_fmt,
            [0, 164, 0, 200],
            "cellStyleXfs is not read"
        );
        assert_eq!(s.format_of(Some(0)), NumberFormat::Builtin(0));
        assert_eq!(s.format_of(Some(0)).code(), "General");
        assert_eq!(
            s.format_of(Some(1)),
            NumberFormat::Custom {
                id: 164,
                code: "_(* #,##0_);_(* \\(#,##0\\);_(* \"-\"??_);_(@_)".to_string()
            }
        );
        // An xf without numFmtId is format 0.
        assert_eq!(s.format_of(Some(2)), NumberFormat::Builtin(0));
    }

    #[test]
    fn test_format_code_entities_decoded() {
        let s = parse_styles(STYLES).expect("styles");
        assert_eq!(
            s.num_fmts[&164],
            "_(* #,##0_);_(* \\(#,##0\\);_(* \"-\"??_);_(@_)"
        );
        // A listed id below 164 is the workbook's own code, not the built-in one.
        assert_eq!(s.num_fmts[&3], "0.0", "listed ids win over built-ins");
        let redefined = Styles {
            num_fmts: BTreeMap::from([(3, "0.0".to_string())]),
            cell_xf_num_fmt: vec![3],
        };
        assert_eq!(redefined.format_of(None).code(), "0.0");
    }

    #[test]
    fn test_cell_without_s_uses_xf0() {
        let s = Styles {
            num_fmts: BTreeMap::new(),
            cell_xf_num_fmt: vec![2, 3],
        };
        assert_eq!(s.format_of(None), NumberFormat::Builtin(2));
        assert_eq!(s.format_of(None).code(), "0.00");
    }

    #[test]
    fn test_bad_style_index_is_not_clamped() {
        let s = parse_styles(STYLES).expect("styles");
        assert_eq!(s.format_of(Some(4)), NumberFormat::BadStyleIndex(4));
        assert_eq!(s.format_of(Some(4)).code(), "bad style index 4");
        // No cellXfs at all: even xf 0 is out of range.
        assert_eq!(
            Styles::default().format_of(None),
            NumberFormat::BadStyleIndex(0)
        );
    }

    #[test]
    fn test_unknown_numfmt_id_is_unknown() {
        let s = parse_styles(STYLES).expect("styles");
        assert_eq!(s.format_of(Some(3)), NumberFormat::Unknown(200));
        assert_eq!(s.format_of(Some(3)).code(), "unknown numFmtId 200");
        // A locale-dependent built-in id (5) that the workbook does not list is unknown too.
        let five = Styles {
            num_fmts: BTreeMap::new(),
            cell_xf_num_fmt: vec![5],
        };
        assert_eq!(five.format_of(None), NumberFormat::Unknown(5));
    }

    #[test]
    fn test_parse_styles_refuses_bad_ids() {
        for bad in [
            r#"<styleSheet><numFmts><numFmt formatCode="0"/></numFmts></styleSheet>"#,
            r#"<styleSheet><numFmts><numFmt numFmtId="164"/></numFmts></styleSheet>"#,
            r#"<styleSheet><cellXfs><xf numFmtId="-1"/></cellXfs></styleSheet>"#,
        ] {
            assert!(parse_styles(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn test_styles_part_name() {
        let rels = r#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
        assert_eq!(styles_part_name(rels).as_deref(), Some("xl/styles.xml"));
        let absolute = rels.replace("Target=\"styles.xml\"", "Target=\"/xl/theme/styles2.xml\"");
        assert_eq!(
            styles_part_name(&absolute).as_deref(),
            Some("xl/theme/styles2.xml")
        );
        assert_eq!(styles_part_name("<Relationships/>"), None);
    }

    #[test]
    fn test_decimal_is_non_integer_exact() {
        for (text, want) in [
            ("0", Some(false)),
            ("2026", Some(false)),
            ("-37.899642999999998", Some(true)),
            ("12.000", Some(false)),
            ("1.25E+2", Some(false)),
            ("1.255E+2", Some(true)),
            ("1.2E+1", Some(false)),
            ("1.23E+1", Some(true)),
            ("1E-3", Some(true)),
            ("5e0", Some(false)),
            (".5", Some(true)),
            ("3.", Some(false)),
            ("+7", Some(false)),
            ("1e-400", Some(true)),
            ("", None),
            ("1.2.3", None),
            ("1e", None),
            ("inf", None),
        ] {
            assert_eq!(decimal_is_non_integer(text), want, "{text:?}");
        }
    }
}
