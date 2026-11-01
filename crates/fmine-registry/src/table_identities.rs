//! Declared PDF table identities (BD-340.S5). Membership, signs, periods and rounding rules are inputs,
//! never inferred from whichever construction makes a table pass. This registry contains no source values.
//! Determinism: Exact (strict parsing and canonical identity/member order).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::sources::{DocumentRow, Fields, SourcesError, fail};
use crate::toml_subset::{TomlValue, parse_subset};

pub const TABLE_IDENTITIES_SCHEMA: &str = "fmine.table-identities.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    Sum,
    RollForward,
}

impl IdentityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::RollForward => "roll_forward",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TermRole {
    Member,
    Opening,
    Move,
}

impl TermRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Opening => "opening",
            Self::Move => "move",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub quantity: String,
    pub period: String,
    /// Coefficient applied to the signed printed value, never a reinterpretation of its printed sign.
    pub sign: i8,
    pub role: TermRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdentity {
    pub id: String,
    pub doc: String,
    pub family: String,
    pub kind: IdentityKind,
    pub total: String,
    pub total_period: String,
    pub unit: String,
    pub basis: String,
    /// `rounded_once` or `component_sum`; one construction for each family across the whole registry.
    pub construction: String,
    /// `closed`, `half_up`, `half_down`, `half_even` or `truncate`; one convention per document.
    pub convention: String,
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdentityRegistry {
    pub issuer: String,
    pub identities: Vec<TableIdentity>,
}

fn key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl TableIdentityRegistry {
    /// Rechecks public structs at a consumer boundary, including caller-constructed declarations.
    pub fn validate(&self, documents: &[DocumentRow]) -> Result<(), SourcesError> {
        if !key(&self.issuer) || self.identities.is_empty() {
            return fail(
                0,
                "an issuer key and at least one table identity are required",
            );
        }
        let mut ids = BTreeSet::new();
        let mut families = BTreeMap::new();
        let mut conventions = BTreeMap::new();
        for identity in &self.identities {
            let bad = |detail: &str| SourcesError {
                line: 0,
                reason: format!("identity {}: {detail}", identity.id),
            };
            if !key(&identity.id) || !ids.insert(&identity.id) || !key(&identity.family) {
                return Err(bad("id and family must be keys; ids must be unique"));
            }
            if documents.iter().filter(|d| d.file == identity.doc).count() != 1 {
                return Err(bad("document must name exactly one registered document"));
            }
            if identity.unit.trim().is_empty()
                || identity.basis.trim().is_empty()
                || identity.total_period.trim().is_empty()
                || !key(&identity.total)
            {
                return Err(bad(
                    "total quantity, period, unit and basis must be explicit",
                ));
            }
            if !matches!(
                identity.construction.as_str(),
                "rounded_once" | "component_sum"
            ) {
                return Err(bad("unknown rounding construction"));
            }
            if !matches!(
                identity.convention.as_str(),
                "closed" | "half_up" | "half_down" | "half_even" | "truncate"
            ) {
                return Err(bad("unknown rounding convention"));
            }
            if families
                .insert(&identity.family, &identity.construction)
                .is_some_and(|old| old != &identity.construction)
            {
                return Err(bad(
                    "a family cannot choose different rounding constructions",
                ));
            }
            if conventions
                .insert(&identity.doc, &identity.convention)
                .is_some_and(|old| old != &identity.convention)
            {
                return Err(bad("a document cannot choose different tie conventions"));
            }
            let mut quantities = BTreeSet::from([&identity.total]);
            if identity.terms.is_empty() {
                return Err(bad("membership must be declared with term rows"));
            }
            for term in &identity.terms {
                if !key(&term.quantity)
                    || !quantities.insert(&term.quantity)
                    || term.period.trim().is_empty()
                    || !matches!(term.sign, -1 | 1)
                {
                    return Err(bad(
                        "terms need unique quantities, explicit periods and signs of -1 or 1",
                    ));
                }
                if (identity.kind == IdentityKind::Sum && term.role != TermRole::Member)
                    || (identity.kind == IdentityKind::RollForward && term.role == TermRole::Member)
                    || (term.role == TermRole::Opening && term.sign != 1)
                {
                    return Err(bad("term role does not match the declared identity kind"));
                }
            }
            if identity.kind == IdentityKind::RollForward
                && (identity
                    .terms
                    .iter()
                    .filter(|t| t.role == TermRole::Opening)
                    .count()
                    != 1
                    || !identity.terms.iter().any(|t| t.role == TermRole::Move))
            {
                return Err(bad(
                    "a roll-forward needs one positive opening and at least one movement",
                ));
            }
        }
        Ok(())
    }
}

/// Parses explicit `[[identity]]` and linked `[[term]]` rows using the strict registry TOML subset.
pub fn parse_table_identities(
    text: &str,
    issuer: &str,
    documents: &[DocumentRow],
) -> Result<TableIdentityRegistry, SourcesError> {
    let parsed = parse_subset(text)?;
    let top = Fields {
        fields: &parsed.top,
        line: 0,
        what: "the top level".to_string(),
    };
    top.only(&["schema", "issuer"])?;
    let (schema, line) = top.string("schema")?;
    if schema != TABLE_IDENTITIES_SCHEMA {
        return fail(line, format!("schema must be {TABLE_IDENTITIES_SCHEMA:?}"));
    }
    let (named, line) = top.string("issuer")?;
    if named != issuer {
        return fail(line, "issuer does not match the sources registry");
    }
    let mut identities = Vec::new();
    let mut terms = Vec::new();
    for row in &parsed.rows {
        let f = Fields::of_row(row);
        match row.table.as_str() {
            "identity" => {
                f.only(&[
                    "id",
                    "doc",
                    "family",
                    "kind",
                    "total",
                    "total_period",
                    "unit",
                    "basis",
                    "construction",
                    "convention",
                ])?;
                let (kind, line) = f.string("kind")?;
                let kind = match kind.as_str() {
                    "sum" => IdentityKind::Sum,
                    "roll_forward" => IdentityKind::RollForward,
                    _ => return fail(line, "kind must be sum or roll_forward"),
                };
                identities.push(TableIdentity {
                    id: f.string("id")?.0,
                    doc: f.string("doc")?.0,
                    family: f.string("family")?.0,
                    kind,
                    total: f.string("total")?.0,
                    total_period: f.string("total_period")?.0,
                    unit: f.string("unit")?.0,
                    basis: f.string("basis")?.0,
                    construction: f.string("construction")?.0,
                    convention: f.string("convention")?.0,
                    terms: Vec::new(),
                });
            }
            "term" => {
                f.only(&["identity", "quantity", "period", "sign", "role"])?;
                let sign = match row.get("sign").map(|v| &v.value) {
                    Some(TomlValue::Int(-1)) => -1,
                    Some(TomlValue::Int(1)) => 1,
                    _ => return fail(row.line, "term sign must be the integer -1 or 1"),
                };
                let (role, line) = f.string("role")?;
                let role = match role.as_str() {
                    "member" => TermRole::Member,
                    "opening" => TermRole::Opening,
                    "move" => TermRole::Move,
                    _ => return fail(line, "role must be member, opening or move"),
                };
                terms.push((
                    f.string("identity")?.0,
                    row.line,
                    Term {
                        quantity: f.string("quantity")?.0,
                        period: f.string("period")?.0,
                        sign,
                        role,
                    },
                ));
            }
            _ => return fail(row.line, format!("unknown table [[{}]]", row.table)),
        }
    }
    for (id, line, term) in terms {
        let Some(identity) = identities.iter_mut().find(|v| v.id == id) else {
            return fail(line, format!("term names unknown identity {id:?}"));
        };
        identity.terms.push(term);
    }
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    for identity in &mut identities {
        identity.terms.sort_by(|a, b| a.quantity.cmp(&b.quantity));
    }
    let registry = TableIdentityRegistry {
        issuer: named,
        identities,
    };
    registry.validate(documents)?;
    Ok(registry)
}

pub fn load_table_identities(
    path: &Path,
    issuer: &str,
    documents: &[DocumentRow],
) -> Result<TableIdentityRegistry, SourcesError> {
    let text = std::fs::read_to_string(path).map_err(|e| SourcesError {
        line: 0,
        reason: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_table_identities(&text, issuer, documents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> DocumentRow {
        DocumentRow {
            file: "Report.pdf".into(),
            dir: "docs".into(),
            kind: "annual_report".into(),
            url: "https://example.test/report.pdf".into(),
            url_basis: "synthetic".into(),
            published_at: "2031-02-01".into(),
            published_at_basis: "synthetic".into(),
            pages: 2,
            bytes: 10,
            sha256: "a".repeat(64),
        }
    }

    fn text() -> String {
        "schema = \"fmine.table-identities.v1\"\nissuer = \"synthetic\"\n\
        [[identity]]\nid = \"reserves\"\ndoc = \"Report.pdf\"\nfamily = \"reserves\"\n\
        kind = \"roll_forward\"\ntotal = \"closing\"\ntotal_period = \"2030FY\"\n\
        unit = \"moz\"\nbasis = \"attributable;contained\"\nconstruction = \"rounded_once\"\nconvention = \"closed\"\n\
        [[term]]\nidentity = \"reserves\"\nquantity = \"opening\"\nperiod = \"2029FY\"\nsign = 1\nrole = \"opening\"\n\
        [[term]]\nidentity = \"reserves\"\nquantity = \"depletion\"\nperiod = \"2030FY\"\nsign = 1\nrole = \"move\"\n".into()
    }

    #[test]
    fn strict_declaration_has_explicit_signed_membership_and_canonical_order() {
        let registry = parse_table_identities(&text(), "synthetic", &[doc()]).unwrap();
        let identity = &registry.identities[0];
        assert_eq!(identity.terms[0].quantity, "depletion");
        assert_eq!(identity.terms[0].sign, 1);
        assert_eq!(identity.terms[1].period, "2029FY");
        for malformed in [
            text().replace("sign = 1", "sign = 0"),
            text().replace("role = \"opening\"", "role = \"move\""),
            text().replace("quantity = \"depletion\"", "quantity = \"closing\""),
            text().replace("Report.pdf", "Unknown.pdf"),
            text().replace("construction =", "unknown ="),
            text().replace("issuer = \"synthetic\"", "issuer = \"other\""),
        ] {
            assert!(parse_table_identities(&malformed, "synthetic", &[doc()]).is_err());
        }
    }

    #[test]
    fn families_and_documents_cannot_pick_rules_per_identity() {
        let mut registry = parse_table_identities(&text(), "synthetic", &[doc()]).unwrap();
        let mut second = registry.identities[0].clone();
        second.id = "other".into();
        second.construction = "component_sum".into();
        registry.identities.push(second);
        assert!(
            registry
                .validate(&[doc()])
                .unwrap_err()
                .reason
                .contains("construction")
        );
        registry.identities[1].construction = "rounded_once".into();
        registry.identities[1].convention = "half_up".into();
        assert!(
            registry
                .validate(&[doc()])
                .unwrap_err()
                .reason
                .contains("conventions")
        );
    }
}
