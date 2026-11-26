//! Conditional rounding hypotheses for matched, same-deck reserve assertions (BD-340.S5).
//! Determinism: Exact (canonical row order and checked rational intersections).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_numerics::exact_feas::Interval;
use fmine_numerics::exact_feas::discover::{Fit, Printed, TieDiscovery, discover_tie_conventions};

use super::geo::{convention_name, fit};
use super::{IssuerReserveRow, VintageComparison, interval, js};

/// Every matched, admitted row for these two documents constrains the same convention pair.
/// Row IDs link the alternative fits back to the register's cited inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentComparison {
    pub documents: (String, String),
    pub row_pairs: Vec<(String, String)>,
    pub result: Result<TieDiscovery, String>,
}

pub(super) fn assess(
    rows: &[IssuerReserveRow],
    comparisons: &[VintageComparison],
) -> Vec<DocumentComparison> {
    let by_id: BTreeMap<_, _> = rows.iter().map(|r| (&r.declaration.id, r)).collect();
    let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for comparison in comparisons {
        let a = by_id[&comparison.left];
        let b = by_id[&comparison.right];
        if a.declaration.doc == b.declaration.doc
            || a.deck.key.is_none()
            || a.deck.key != b.deck.key
        {
            continue;
        }
        let (left, right) = if a.declaration.doc < b.declaration.doc {
            (a, b)
        } else {
            (b, a)
        };
        groups
            .entry((left.declaration.doc.clone(), right.declaration.doc.clone()))
            .or_default()
            .push((left, right));
    }
    groups
        .into_iter()
        .map(|(documents, mut pairs)| {
            pairs.sort_by_key(|(a, b)| (&a.declaration.id, &b.declaration.id));
            let row_pairs: Vec<_> = pairs
                .iter()
                .map(|(a, b)| (a.declaration.id.clone(), b.declaration.id.clone()))
                .collect();
            let mut seen = BTreeSet::new();
            let mut printed = Vec::new();
            for (a, b) in pairs {
                for row in [a, b] {
                    seen.insert(&row.declaration.id);
                    printed.push(Printed {
                        document: row.declaration.doc.clone(),
                        quantity: a.declaration.id.clone(),
                        printed: row.source.cell.printed,
                        step: row.source.cell.step,
                    });
                }
            }
            let result = if seen.len() == printed.len() {
                discover_tie_conventions(&printed).map_err(|e| format!("{e:?}"))
            } else {
                Err("multiple reserve assertions for one matched quantity in a document".into())
            };
            DocumentComparison {
                documents,
                row_pairs,
                result,
            }
        })
        .collect()
}

impl DocumentComparison {
    pub(super) fn json(&self) -> String {
        let rows = self
            .row_pairs
            .iter()
            .map(|(a, b)| format!("{{\"left\":{},\"right\":{}}}", js(a), js(b)))
            .collect::<Vec<_>>()
            .join(",");
        let mut body = format!(
            "{{\"left_document\":{},\"right_document\":{},\"row_pairs\":[{rows}]",
            js(&self.documents.0),
            js(&self.documents.1)
        );
        match &self.result {
            Err(reason) => {
                let _ = write!(body, ",\"refusal\":{}", js(reason));
            }
            Ok(d) => {
                let assignments = d
                    .assignments
                    .iter()
                    .map(|a| {
                        format!(
                            "{{\"left\":{},\"right\":{},\"fit\":{}}}",
                            js(convention_name(a.conventions[0].1)),
                            js(convention_name(a.conventions[1].1)),
                            js(fit(a.fit))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let ranges = d
                    .closed
                    .ranges
                    .iter()
                    .map(|(id, range)| {
                        let overlap = match (range.lo, range.hi) {
                            (Some(lo), Some(hi))
                                if lo.at.checked_cmp(hi.at).is_ok_and(|o| o.is_le()) =>
                            {
                                interval(&Interval {
                                    lo: lo.at,
                                    hi: hi.at,
                                    lo_strict: lo.strict,
                                    hi_strict: hi.strict,
                                })
                            }
                            _ => "null".into(),
                        };
                        format!("{{\"left_row\":{},\"overlap\":{overlap}}}", js(id))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let _ = write!(
                    body,
                    ",\"closed_fit\":{},\"closed_ranges\":[{ranges}],\"strict_assignments\":[{assignments}],\"conflicting_left_rows\":{}",
                    js(fit(d.closed.fit)),
                    super::list(&d.conflicts)
                );
            }
        }
        body.push('}');
        body
    }

    pub(super) fn markdown(&self, text: impl Fn(&str) -> String) -> String {
        let mut out = format!(
            "\n#### {} and {}\n\nMatched reserve rows: {}.\n\n",
            text(&self.documents.0),
            text(&self.documents.1),
            self.row_pairs
                .iter()
                .map(|(a, b)| format!("{} / {}", text(a), text(b)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        match &self.result {
            Err(reason) => {
                let _ = writeln!(out, "Rounding discovery unavailable: {}.", text(reason));
            }
            Ok(d) => {
                let _ = writeln!(
                    out,
                    "Closed half-unit intervals: {}. The following strict convention pairs admit a common value for every matched quantity.\n",
                    fit(d.closed.fit)
                );
                let mut n = 0;
                for a in d.assignments.iter().filter(|a| a.fit != Fit::Empty) {
                    n += 1;
                    let _ = writeln!(
                        out,
                        "- {} / {}: {}",
                        convention_name(a.conventions[0].1),
                        convention_name(a.conventions[1].1),
                        fit(a.fit)
                    );
                }
                if n == 0 {
                    out.push_str("No strict convention pair fits all matched quantities.\n");
                }
            }
        }
        out
    }
}
