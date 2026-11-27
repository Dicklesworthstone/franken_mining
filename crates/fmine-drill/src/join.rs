//! From/to overlap joins per hole (`BD-165`, plan §12.2 "Interval joins").
//! Determinism: Exact (integer depths, sorting and comparisons only).
//!
//! Two sets of from/to spans along one hole, such as assay samples and lithology intervals, are joined by a sweep
//! over their starts. Spans are taken in order of `from_mm`; a span still open on the other side when one starts
//! overlaps it, and a span that has ended is dropped before it is looked at again. Each overlapping pair is found
//! once, when the later of the two starts. The work is `O((n + m) log(n + m) + k)` for `k` overlaps.
//!
//! An overlap has positive length: spans that only touch (`[0, 10)` and `[10, 20)`) do not overlap. The result is
//! sorted by left index, then right index, so it is the same whatever order the sweep met the spans in.

use std::collections::BTreeMap;

use crate::records::{DrillError, Interval};

/// A from/to span along a hole, mm, with `to_mm` past `from_mm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// From depth, mm.
    pub from_mm: u64,
    /// To depth, mm.
    pub to_mm: u64,
}

impl Span {
    /// The span of an interval record.
    #[must_use]
    pub const fn of(interval: &Interval) -> Self {
        Self {
            from_mm: interval.from_mm,
            to_mm: interval.to_mm,
        }
    }

    /// Length, mm.
    #[must_use]
    pub const fn length_mm(&self) -> u64 {
        self.to_mm - self.from_mm
    }
}

/// One overlapping pair: indices into the two inputs and the shared span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Overlap {
    /// Index into the left input.
    pub left: usize,
    /// Index into the right input.
    pub right: usize,
    /// The part of the hole both cover.
    pub span: Span,
}

fn check(side: &str, spans: &[Span]) -> Result<(), DrillError> {
    match spans.iter().position(|s| s.to_mm <= s.from_mm) {
        None => Ok(()),
        Some(i) => Err(DrillError::invalid(
            side,
            format!(
                "span {i} runs from {} mm to {} mm; a span ends past where it starts",
                spans[i].from_mm, spans[i].to_mm
            ),
        )),
    }
}

/// Every pair of a left and a right span that overlap, sorted by left index, then right index.
///
/// # Errors
/// [`DrillError::Invalid`] for a span whose `to_mm` is not past its `from_mm`.
pub fn overlaps(left: &[Span], right: &[Span]) -> Result<Vec<Overlap>, DrillError> {
    check("join.left", left)?;
    check("join.right", right)?;
    // Starts in order; at one depth the left spans come first, which only decides which side finds the pair.
    let mut starts: Vec<(u64, bool, usize)> = Vec::with_capacity(left.len() + right.len());
    starts.extend(left.iter().enumerate().map(|(i, s)| (s.from_mm, false, i)));
    starts.extend(right.iter().enumerate().map(|(i, s)| (s.from_mm, true, i)));
    starts.sort_unstable();
    let mut open_left: Vec<usize> = Vec::new();
    let mut open_right: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for (from_mm, is_right, i) in starts {
        let (this, mine, theirs, other) = if is_right {
            (right[i], &mut open_right, &mut open_left, left)
        } else {
            (left[i], &mut open_left, &mut open_right, right)
        };
        theirs.retain(|&j| other[j].to_mm > from_mm);
        for &j in theirs.iter() {
            let span = Span {
                from_mm,
                to_mm: this.to_mm.min(other[j].to_mm),
            };
            out.push(if is_right {
                Overlap {
                    left: j,
                    right: i,
                    span,
                }
            } else {
                Overlap {
                    left: i,
                    right: j,
                    span,
                }
            });
        }
        mine.push(i);
    }
    out.sort_unstable();
    Ok(out)
}

/// Every pair `(i, j)`, `i < j`, of spans in one set that overlap, sorted by `i`, then `j`: for example the
/// intervals of one logger that claim the same core.
///
/// # Errors
/// [`DrillError::Invalid`] for a span whose `to_mm` is not past its `from_mm`.
pub fn self_overlaps(spans: &[Span]) -> Result<Vec<Overlap>, DrillError> {
    check("join.spans", spans)?;
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_unstable_by_key(|&i| (spans[i].from_mm, i));
    let mut open: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for i in order {
        let this = spans[i];
        open.retain(|&j| spans[j].to_mm > this.from_mm);
        for &j in &open {
            out.push(Overlap {
                left: i.min(j),
                right: i.max(j),
                span: Span {
                    from_mm: this.from_mm,
                    to_mm: this.to_mm.min(spans[j].to_mm),
                },
            });
        }
        open.push(i);
    }
    out.sort_unstable();
    Ok(out)
}

/// An overlap of two interval records on one hole.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HoleOverlap {
    /// The hole.
    pub hole_id: String,
    /// The overlap, with indices into the two record slices.
    pub overlap: Overlap,
}

/// Join two sets of interval records hole by hole: every pair on the same hole that overlaps, sorted by hole,
/// then left index, then right index. Records on different holes never overlap.
///
/// # Errors
/// [`DrillError::Invalid`] for an interval whose `to_mm` is not past its `from_mm`.
pub fn join_intervals(
    left: &[Interval],
    right: &[Interval],
) -> Result<Vec<HoleOverlap>, DrillError> {
    let mut holes: BTreeMap<&str, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (i, r) in left.iter().enumerate() {
        holes.entry(r.hole_id.as_str()).or_default().0.push(i);
    }
    for (i, r) in right.iter().enumerate() {
        holes.entry(r.hole_id.as_str()).or_default().1.push(i);
    }
    let mut out = Vec::new();
    for (hole_id, (l, r)) in holes {
        if l.is_empty() || r.is_empty() {
            continue;
        }
        let ls: Vec<Span> = l.iter().map(|&i| Span::of(&left[i])).collect();
        let rs: Vec<Span> = r.iter().map(|&i| Span::of(&right[i])).collect();
        for o in overlaps(&ls, &rs)? {
            out.push(HoleOverlap {
                hole_id: hole_id.to_string(),
                overlap: Overlap {
                    left: l[o.left],
                    right: r[o.right],
                    span: o.span,
                },
            });
        }
    }
    Ok(out)
}
