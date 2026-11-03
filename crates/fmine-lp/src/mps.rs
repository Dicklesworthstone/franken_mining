//! MPS input and output for [`LpProblem`] (bd-j2ay, plan §15.4: "MPS I/O"; FrankenMining "exports MPS plus a
//! certificate request").
//! Determinism: BitwisePortable (decimal text to `f64` by Rust's correctly rounded parser, `f64` to text in its
//! shortest round-trip form; the only arithmetic is negation and the range sums, each checked exact when written).
//!
//! **Reading** takes free-format MPS: fields are separated by spaces or tabs, so names hold neither. A line that
//! starts in column 1 opens a section; data lines start with a space or tab. Lines starting with `*` and blank lines
//! are skipped.
//! - `NAME` takes the rest of its line. `OBJSENSE` takes `MIN`, `MAX`, `MINIMIZE` or `MAXIMIZE` on its own line or
//!   the next.
//! - `ROWS`: `N`, `L`, `G` or `E`, then a name. The first `N` row is the objective. A later `N` row is kept as a free
//!   row, which constrains nothing.
//! - `COLUMNS`: a column, then one or two row-value pairs. Each column's lines are contiguous.
//! - `RHS`, `RANGES`: an optional set name, then one or two row-value pairs. A right-hand side `b` on the objective
//!   row adds the constant `−b` to the objective, as CPLEX and HiGHS read it.
//! - `RHS`, `RANGES` and `BOUNDS` each read one set: the lines that name a set name the same one.
//! - A range `R` on a row with right-hand side `b`: `G` gives `[b, b + |R|]`, `L` gives `[b − |R|, b]`, and `E`
//!   gives `[b, b + R]` for `R > 0` and `[b + R, b]` for `R < 0`.
//! - `BOUNDS`: a type, an optional set name, a column and, for `UP`, `LO` and `FX`, a value. `FR` frees the column,
//!   `MI` sets its lower bound to `−∞` and `PL` its upper bound to `+∞`. A column's default is `[0, +∞)`.
//! - `ENDATA` ends the file.
//!
//! A maximisation is returned as the minimisation of `−c`, `LpProblem`'s form, with [`MpsModel::maximise`] set; the
//! objective constant keeps the file's sense.
//!
//! **Writing** ([`write`]) gives free-format MPS that [`read`] takes back to the same `LpProblem` bit for bit:
//! - numbers in their shortest round-trip form;
//! - rows named `R0`, `R1`, … and columns `C0`, `C1`, …, with the objective row `OBJ`;
//! - a row with two finite bounds as `G` or `L` with a range, whichever reads back exactly;
//! - a row with no finite bound as `N`.

use std::collections::{HashMap, HashSet};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::sparse::Csr;

use crate::pdlp::LpProblem;

fn refuse(line: usize, detail: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("MPS line {line}: {detail}"),
    )
}

/// A linear program read from MPS.
#[derive(Debug, Clone)]
pub struct MpsModel {
    /// The `NAME` line's text, or empty.
    pub name: String,
    /// The objective row's name.
    pub objective: String,
    /// The constraint rows' names, in the file's order, the objective left out: row `i` of `problem`.
    pub rows: Vec<String>,
    /// The columns' names, in the file's order: column `j` of `problem`.
    pub columns: Vec<String>,
    /// Whether the file maximises. `problem` then minimises `−c`.
    pub maximise: bool,
    /// The objective constant, in the file's sense: the file's objective is `c·x + offset`.
    pub offset: f64,
    /// The constraints and costs, as a minimisation.
    pub problem: LpProblem,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Objsense,
    Rows,
    Columns,
    Rhs,
    Ranges,
    Bounds,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Free,
    Le,
    Ge,
    Eq,
}

struct Builder {
    name: String,
    maximise: bool,
    objective: Option<String>,
    rows: Vec<(String, Kind)>,
    row_of: HashMap<String, usize>,
    columns: Vec<String>,
    column_of: HashMap<String, usize>,
    costs: Vec<f64>,
    entries: Vec<(usize, usize, f64)>,
    seen: HashSet<(usize, usize)>,
    cost_seen: Vec<bool>,
    rhs: Vec<Option<f64>>,
    offset: Option<f64>,
    ranges: Vec<Option<f64>>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    lower_set: Vec<bool>,
    sets: [Option<String>; 3],
}

/// Where a row-value pair's row name points.
enum Target {
    Objective,
    Row(usize),
}

impl Builder {
    fn target(&self, line: usize, row: &str) -> Result<Target, Refusal> {
        if self.objective.as_deref() == Some(row) {
            return Ok(Target::Objective);
        }
        self.row_of
            .get(row)
            .map(|&i| Target::Row(i))
            .ok_or_else(|| refuse(line, format!("no row is called \"{row}\"")))
    }

    fn column(&self, line: usize, name: &str) -> Result<usize, Refusal> {
        self.column_of
            .get(name)
            .copied()
            .ok_or_else(|| refuse(line, format!("no column is called \"{name}\"")))
    }

    /// The set name of a `RHS`, `RANGES` or `BOUNDS` line: the same on every line of its section that gives one.
    fn set(&mut self, line: usize, which: usize, name: Option<&str>) -> Result<(), Refusal> {
        let Some(name) = name else {
            return Ok(());
        };
        match &self.sets[which] {
            None => {
                self.sets[which] = Some(name.to_string());
                Ok(())
            }
            Some(s) if s == name => Ok(()),
            Some(s) => Err(refuse(
                line,
                format!(
                    "a second {} set \"{name}\" after \"{s}\"; one set is read",
                    ["RHS", "RANGES", "BOUNDS"][which]
                ),
            )),
        }
    }

    fn row(&mut self, line: usize, kind: &str, name: &str) -> Result<(), Refusal> {
        let kind = match kind {
            "N" if self.objective.is_none() => {
                if self.row_of.contains_key(name) {
                    return Err(refuse(line, format!("a second row called \"{name}\"")));
                }
                self.objective = Some(name.to_string());
                return Ok(());
            }
            "N" => Kind::Free,
            "L" => Kind::Le,
            "G" => Kind::Ge,
            "E" => Kind::Eq,
            other => {
                return Err(refuse(
                    line,
                    format!("row type \"{other}\" is not N, L, G or E"),
                ));
            }
        };
        if self.objective.as_deref() == Some(name) || self.row_of.contains_key(name) {
            return Err(refuse(line, format!("a second row called \"{name}\"")));
        }
        self.row_of.insert(name.to_string(), self.rows.len());
        self.rows.push((name.to_string(), kind));
        self.rhs.push(None);
        self.ranges.push(None);
        Ok(())
    }

    fn entry(&mut self, line: usize, column: &str, row: &str, value: f64) -> Result<(), Refusal> {
        let j = match self.columns.last() {
            Some(last) if last == column => self.columns.len() - 1,
            _ => {
                if self.column_of.contains_key(column) {
                    return Err(refuse(
                        line,
                        format!(
                            "column \"{column}\" appears again after other columns; each column's lines are \
                             contiguous"
                        ),
                    ));
                }
                self.column_of
                    .insert(column.to_string(), self.columns.len());
                self.columns.push(column.to_string());
                self.costs.push(0.0);
                self.cost_seen.push(false);
                self.lower.push(0.0);
                self.upper.push(f64::INFINITY);
                self.lower_set.push(false);
                self.columns.len() - 1
            }
        };
        match self.target(line, row)? {
            Target::Objective => {
                if std::mem::replace(&mut self.cost_seen[j], true) {
                    return Err(refuse(
                        line,
                        format!("column \"{column}\" has a second objective entry"),
                    ));
                }
                self.costs[j] = value;
            }
            Target::Row(i) => {
                if !self.seen.insert((i, j)) {
                    return Err(refuse(
                        line,
                        format!("column \"{column}\" has a second entry in row \"{row}\""),
                    ));
                }
                self.entries.push((i, j, value));
            }
        }
        Ok(())
    }

    fn rhs(&mut self, line: usize, row: &str, value: f64) -> Result<(), Refusal> {
        let slot = match self.target(line, row)? {
            Target::Objective => &mut self.offset,
            Target::Row(i) if self.rows[i].1 == Kind::Free => {
                return Err(refuse(
                    line,
                    format!("free row \"{row}\" has a right-hand side"),
                ));
            }
            Target::Row(i) => &mut self.rhs[i],
        };
        if slot.replace(value).is_some() {
            return Err(refuse(
                line,
                format!("row \"{row}\" has a second right-hand side"),
            ));
        }
        Ok(())
    }

    fn range(&mut self, line: usize, row: &str, value: f64) -> Result<(), Refusal> {
        let i = match self.target(line, row)? {
            Target::Row(i) if self.rows[i].1 != Kind::Free => i,
            _ => {
                return Err(refuse(
                    line,
                    format!("row \"{row}\" is an N row and takes no range"),
                ));
            }
        };
        if self.ranges[i].replace(value).is_some() {
            return Err(refuse(line, format!("row \"{row}\" has a second range")));
        }
        Ok(())
    }

    fn bound(
        &mut self,
        line: usize,
        kind: &str,
        column: &str,
        value: Option<f64>,
    ) -> Result<(), Refusal> {
        let j = self.column(line, column)?;
        match (kind, value) {
            ("UP", Some(v)) => {
                if v < 0.0 && !self.lower_set[j] {
                    return Err(refuse(
                        line,
                        format!(
                            "column \"{column}\" has the negative upper bound {v} over the default lower bound 0; \
                             readers disagree on it, so state the lower bound first (LO or MI)"
                        ),
                    ));
                }
                self.upper[j] = v;
            }
            ("LO", Some(v)) => {
                self.lower[j] = v;
                self.lower_set[j] = true;
            }
            ("FX", Some(v)) => {
                self.lower[j] = v;
                self.upper[j] = v;
                self.lower_set[j] = true;
            }
            ("FR", None) => {
                self.lower[j] = f64::NEG_INFINITY;
                self.upper[j] = f64::INFINITY;
                self.lower_set[j] = true;
            }
            ("MI", None) => {
                self.lower[j] = f64::NEG_INFINITY;
                self.lower_set[j] = true;
            }
            ("PL", None) => self.upper[j] = f64::INFINITY,
            _ => {
                return Err(refuse(
                    line,
                    format!("bound type \"{kind}\" with this many fields is not read"),
                ));
            }
        }
        Ok(())
    }

    fn finish(self, line: usize) -> Result<MpsModel, Refusal> {
        let objective = self
            .objective
            .ok_or_else(|| refuse(line, "there is no objective (N) row"))?;
        let m = self.rows.len();
        let mut row_lo = Vec::with_capacity(m);
        let mut row_hi = Vec::with_capacity(m);
        for (i, (name, kind)) in self.rows.iter().enumerate() {
            let b = self.rhs[i].unwrap_or(0.0);
            let (lo, hi) = match (kind, self.ranges[i]) {
                (Kind::Free, _) => (f64::NEG_INFINITY, f64::INFINITY),
                (Kind::Le, None) => (f64::NEG_INFINITY, b),
                (Kind::Ge, None) => (b, f64::INFINITY),
                (Kind::Eq, None) => (b, b),
                (Kind::Le, Some(r)) => (b - r.abs(), b),
                (Kind::Ge, Some(r)) => (b, b + r.abs()),
                (Kind::Eq, Some(r)) if r >= 0.0 => (b, b + r),
                (Kind::Eq, Some(r)) => (b + r, b),
            };
            if self.ranges[i].is_some() && !(lo.is_finite() && hi.is_finite()) {
                return Err(refuse(
                    line,
                    format!("row \"{name}\"'s range overflows: [{lo}, {hi}]"),
                ));
            }
            row_lo.push(lo);
            row_hi.push(hi);
        }
        let n = self.columns.len();
        let mut entries = self.entries;
        entries.sort_unstable_by_key(|&(i, j, _)| (i, j));
        let mut ptr = vec![0usize; m + 1];
        for &(i, _, _) in &entries {
            ptr[i + 1] += 1;
        }
        for i in 0..m {
            ptr[i + 1] += ptr[i];
        }
        let (idx, val) = entries.iter().map(|&(_, j, v)| (j, v)).unzip();
        let a = Csr::from_parts(m, n, ptr, idx, val)
            .map_err(|e| refuse(line, format!("the matrix does not assemble: {e:?}")))?;
        let sign = if self.maximise { -1.0 } else { 1.0 };
        let costs = self.costs.iter().map(|&c| sign * c).collect();
        let problem = LpProblem::new(costs, &a, row_lo, row_hi, self.lower, self.upper)?;
        Ok(MpsModel {
            name: self.name,
            objective,
            rows: self.rows.into_iter().map(|(name, _)| name).collect(),
            columns: self.columns,
            maximise: self.maximise,
            offset: self.offset.map_or(0.0, |b| -b),
            problem,
        })
    }
}

fn number(line: usize, text: &str) -> Result<f64, Refusal> {
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(refuse(line, format!("\"{text}\" is not a finite number"))),
    }
}

fn sense(line: usize, word: &str) -> Result<bool, Refusal> {
    match word {
        "MIN" | "MINIMIZE" | "MINIMISE" => Ok(false),
        "MAX" | "MAXIMIZE" | "MAXIMISE" => Ok(true),
        other => Err(refuse(
            line,
            format!("OBJSENSE \"{other}\" is not MIN or MAX"),
        )),
    }
}

/// Reads free-format MPS (see the module notes).
///
/// # Errors
/// `FMINE-REF-506`, naming the line, for:
/// - an unknown section, a data line outside a section, a missing `ENDATA`, or text after it;
/// - a row type other than `N`, `L`, `G` or `E`, a row or column named twice, or a name no row or column has;
/// - integer markers or bound types (`BV`, `LI`, `UI`, `SC`): this reads linear programs;
/// - a second entry for one row and column, a second objective entry, right-hand side or range for one row, a
///   right-hand side on a free row, or a range on an `N` row;
/// - a second `RHS`, `RANGES` or `BOUNDS` set;
/// - a negative `UP` bound on a column whose lower bound is still the default 0, which readers take differently;
/// - a field that is not a finite number where one is due, or a line with the wrong number of fields;
/// - and, passed on, `LpProblem::new`'s refusals, such as a lower bound above the upper.
pub fn read(text: &str) -> Result<MpsModel, Refusal> {
    let mut b = Builder {
        name: String::new(),
        maximise: false,
        objective: None,
        rows: Vec::new(),
        row_of: HashMap::new(),
        columns: Vec::new(),
        column_of: HashMap::new(),
        costs: Vec::new(),
        entries: Vec::new(),
        seen: HashSet::new(),
        cost_seen: Vec::new(),
        rhs: Vec::new(),
        offset: None,
        ranges: Vec::new(),
        lower: Vec::new(),
        upper: Vec::new(),
        lower_set: Vec::new(),
        sets: [None, None, None],
    };
    let mut section = Section::None;
    let mut sense_due = false;
    for (k, raw) in text.lines().enumerate() {
        let line = k + 1;
        if raw.starts_with('*') || raw.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = raw.split_whitespace().collect();
        if !raw.starts_with([' ', '\t']) {
            if sense_due {
                return Err(refuse(line, "OBJSENSE has no MIN or MAX"));
            }
            section = match fields[0] {
                "NAME" => {
                    b.name = raw[4..].trim().to_string();
                    Section::None
                }
                "OBJSENSE" => match fields.get(1) {
                    Some(w) => {
                        b.maximise = sense(line, w)?;
                        Section::None
                    }
                    None => {
                        sense_due = true;
                        Section::Objsense
                    }
                },
                "ROWS" => Section::Rows,
                "COLUMNS" => Section::Columns,
                "RHS" => Section::Rhs,
                "RANGES" => Section::Ranges,
                "BOUNDS" => Section::Bounds,
                "ENDATA" => {
                    if let Some(extra) = text
                        .lines()
                        .enumerate()
                        .skip(line)
                        .find(|(_, l)| !l.trim().is_empty() && !l.starts_with('*'))
                    {
                        return Err(refuse(extra.0 + 1, "text after ENDATA"));
                    }
                    return b.finish(line);
                }
                other => {
                    return Err(refuse(line, format!("unknown section \"{other}\"")));
                }
            };
            continue;
        }
        match section {
            Section::None => return Err(refuse(line, "a data line outside any section")),
            Section::Objsense => {
                if !sense_due || fields.len() != 1 {
                    return Err(refuse(line, "OBJSENSE takes one word"));
                }
                b.maximise = sense(line, fields[0])?;
                sense_due = false;
            }
            Section::Rows => match fields.as_slice() {
                [kind, name] => b.row(line, kind, name)?,
                _ => return Err(refuse(line, "a ROWS line is a type and a name")),
            },
            Section::Columns => {
                if fields.get(1) == Some(&"'MARKER'") || fields.get(1) == Some(&"MARKER") {
                    return Err(refuse(
                        line,
                        "integer markers: this reader takes linear programs only",
                    ));
                }
                match fields.as_slice() {
                    [column, pairs @ ..]
                        if !pairs.is_empty() && pairs.len() % 2 == 0 && pairs.len() <= 4 =>
                    {
                        for pair in pairs.chunks(2) {
                            b.entry(line, column, pair[0], number(line, pair[1])?)?;
                        }
                    }
                    _ => {
                        return Err(refuse(
                            line,
                            "a COLUMNS line is a column and one or two row-value pairs",
                        ));
                    }
                }
            }
            Section::Rhs | Section::Ranges => {
                let which = usize::from(section == Section::Ranges);
                let (set, pairs) = match fields.len() {
                    2 | 4 => (None, &fields[..]),
                    3 | 5 => (Some(fields[0]), &fields[1..]),
                    _ => {
                        return Err(refuse(
                            line,
                            "an RHS or RANGES line is an optional set name and one or two row-value pairs",
                        ));
                    }
                };
                b.set(line, which, set)?;
                for pair in pairs.chunks(2) {
                    let v = number(line, pair[1])?;
                    if which == 0 {
                        b.rhs(line, pair[0], v)?;
                    } else {
                        b.range(line, pair[0], v)?;
                    }
                }
            }
            Section::Bounds => {
                let kind = fields[0];
                if matches!(kind, "BV" | "LI" | "UI" | "SC") {
                    return Err(refuse(
                        line,
                        format!(
                            "integer bound type \"{kind}\": this reader takes linear programs only"
                        ),
                    ));
                }
                let valued = matches!(kind, "UP" | "LO" | "FX");
                let (set, column, value) = match (valued, fields.as_slice()) {
                    (true, [_, column, v]) => (None, *column, Some(number(line, v)?)),
                    (true, [_, set, column, v]) => (Some(*set), *column, Some(number(line, v)?)),
                    (false, [_, column]) => (None, *column, None),
                    (false, [_, set, column]) => (Some(*set), *column, None),
                    _ => {
                        return Err(refuse(
                            line,
                            "a BOUNDS line is a type, an optional set name, a column and, for UP, LO and FX, a \
                             value",
                        ));
                    }
                };
                b.set(line, 2, set)?;
                b.bound(line, kind, column, value)?;
            }
        }
    }
    Err(refuse(text.lines().count(), "there is no ENDATA"))
}

/// Writes `problem` as free-format MPS under `name` (see the module notes); [`read`] takes it back bit for bit.
///
/// # Errors
/// `FMINE-REF-506` when `name` holds a space or tab, or when a row with two different finite bounds has a range
/// that reads back inexactly both as `G` and as `L`.
pub fn write(name: &str, problem: &LpProblem) -> Result<String, Refusal> {
    use std::fmt::Write as _;
    if name.contains([' ', '\t']) {
        return Err(refuse(
            1,
            format!("the name \"{name}\" holds a space or tab"),
        ));
    }
    let mut rows = String::from(" N  OBJ\n");
    let mut rhs = String::new();
    let mut ranges = String::new();
    for (i, (&lo, &hi)) in problem.row_lo.iter().zip(&problem.row_hi).enumerate() {
        let (kind, b, range) = if lo.is_infinite() && hi.is_infinite() {
            ('N', None, None)
        } else if lo.to_bits() == hi.to_bits() {
            ('E', Some(lo), None)
        } else if lo.is_infinite() {
            ('L', Some(hi), None)
        } else if hi.is_infinite() {
            ('G', Some(lo), None)
        } else {
            let r = hi - lo;
            if (lo + r).to_bits() == hi.to_bits() {
                ('G', Some(lo), Some(r))
            } else if (hi - r).to_bits() == lo.to_bits() {
                ('L', Some(hi), Some(r))
            } else {
                return Err(refuse(
                    i + 1,
                    format!(
                        "row {i}'s bounds [{lo:?}, {hi:?}] have no range that reads back exactly"
                    ),
                ));
            }
        };
        let _ = writeln!(rows, " {kind}  R{i}");
        if let Some(b) = b.filter(|b| b.to_bits() != 0) {
            let _ = writeln!(rhs, "    RHS  R{i}  {b:?}");
        }
        if let Some(r) = range {
            let _ = writeln!(ranges, "    RNG  R{i}  {r:?}");
        }
    }
    let mut columns = String::new();
    let mut bounds = String::new();
    for j in 0..problem.c.len() {
        let c = problem.c[j];
        let (idx, val) = problem.at.row(j);
        if c.to_bits() != 0 || idx.is_empty() {
            let _ = writeln!(columns, "    C{j}  OBJ  {c:?}");
        }
        for (&i, &v) in idx.iter().zip(val) {
            let _ = writeln!(columns, "    C{j}  R{i}  {v:?}");
        }
        let (lo, hi) = (problem.col_lo[j], problem.col_hi[j]);
        if lo.is_infinite() && hi.is_infinite() {
            let _ = writeln!(bounds, " FR BND  C{j}");
        } else if lo.to_bits() == hi.to_bits() {
            let _ = writeln!(bounds, " FX BND  C{j}  {lo:?}");
        } else {
            if lo.is_infinite() {
                let _ = writeln!(bounds, " MI BND  C{j}");
            } else if lo.to_bits() != 0 {
                let _ = writeln!(bounds, " LO BND  C{j}  {lo:?}");
            }
            if hi.is_finite() {
                let _ = writeln!(bounds, " UP BND  C{j}  {hi:?}");
            }
        }
    }
    let mut out = format!("NAME {name}\nROWS\n{rows}COLUMNS\n{columns}RHS\n{rhs}");
    if !ranges.is_empty() {
        let _ = write!(out, "RANGES\n{ranges}");
    }
    if !bounds.is_empty() {
        let _ = write!(out, "BOUNDS\n{bounds}");
    }
    out.push_str("ENDATA\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdlp::{PdlpOptions, solve};
    use fmine_numerics::sparse::Coo;

    /// Every section and bound type, with the set names left out on two lines.
    const EVERY: &str = "\
* A comment, then a NAME with words in it.
NAME          EVERY  with words
OBJSENSE
    MAX
ROWS
 N  COST
 L  LIM1
 G  LIM2
 E  MYEQN
 E  RNGE
 L  RNGL
 G  RNGG
 E  RNGN
 N  FREE
COLUMNS
    X1  COST  1.0  LIM1  1.0
    X1  LIM2  1.0
    X2  COST  2.0  LIM1  1.0
    X2  MYEQN  -1.0
    X3  COST  -1.0  MYEQN  1.0
\tX3  RNGE  1  RNGL  2
    X4  RNGG  3  RNGN  4
    X4  FREE  5
    X5  COST  0
    X6  COST  3
    X7  RNGE  1

RHS
    RHS  COST  -2.5  LIM1  4.0
    RHS  LIM2  1.0  MYEQN  7.0
    RNGE  5  RNGL  6
    RHS  RNGG  7  RNGN  8
RANGES
    RNG  RNGE  2  RNGL  -3
    RNG  RNGG  -4  RNGN  -5
BOUNDS
 UP BND  X1  4.0
 MI BND  X2
 UP BND  X2  -1
 LO BND  X3  -2
 FX BND  X4  3.5
 FR BND  X5
 LO BND  X6  1
 PL BND  X6
 LO X7  -1
 UP BND  X7  2
ENDATA
";

    fn csr(rows: usize, cols: usize, entries: &[(usize, usize, f64)]) -> Csr {
        let mut coo = Coo::new(rows, cols).unwrap();
        for &(r, c, v) in entries {
            coo.push(r, c, v).unwrap();
        }
        coo.assemble()
    }

    fn bits(v: &[f64]) -> Vec<u64> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    /// The same costs, bounds and matrix, bit for bit.
    fn assert_same(p: &LpProblem, q: &LpProblem) {
        assert_eq!(bits(&p.c), bits(&q.c), "costs");
        assert_eq!(bits(&p.row_lo), bits(&q.row_lo), "row lower bounds");
        assert_eq!(bits(&p.row_hi), bits(&q.row_hi), "row upper bounds");
        assert_eq!(bits(&p.col_lo), bits(&q.col_lo), "column lower bounds");
        assert_eq!(bits(&p.col_hi), bits(&q.col_hi), "column upper bounds");
        assert_eq!((p.rows(), p.cols(), p.nnz()), (q.rows(), q.cols(), q.nnz()));
        for j in 0..p.cols() {
            let (pi, pv) = p.at.row(j);
            let (qi, qv) = q.at.row(j);
            assert_eq!((pi, bits(pv)), (qi, bits(qv)), "column {j}");
        }
    }

    fn refused(text: &str, why: &str) {
        let e = read(text).map(drop).unwrap_err();
        assert_eq!(e.code, "FMINE-REF-506", "{e:?}");
        assert!(e.what.contains(why), "wanted {why:?} in {}", e.what);
    }

    #[test]
    fn every_section_and_bound_type_reads_as_written() {
        let m = read(EVERY).unwrap();
        assert_eq!(m.name, "EVERY  with words");
        assert_eq!(m.objective, "COST");
        assert_eq!(
            m.rows,
            [
                "LIM1", "LIM2", "MYEQN", "RNGE", "RNGL", "RNGG", "RNGN", "FREE"
            ]
        );
        assert_eq!(m.columns, ["X1", "X2", "X3", "X4", "X5", "X6", "X7"]);
        assert!(m.maximise);
        assert_eq!(m.offset, 2.5);
        let inf = f64::INFINITY;
        let want = LpProblem::new(
            vec![-1.0, -2.0, 1.0, -0.0, -0.0, -3.0, -0.0],
            &csr(
                8,
                7,
                &[
                    (0, 0, 1.0),
                    (0, 1, 1.0),
                    (1, 0, 1.0),
                    (2, 1, -1.0),
                    (2, 2, 1.0),
                    (3, 2, 1.0),
                    (3, 6, 1.0),
                    (4, 2, 2.0),
                    (5, 3, 3.0),
                    (6, 3, 4.0),
                    (7, 3, 5.0),
                ],
            ),
            // E with +2 gives [5, 7]; L with -3 gives [3, 6]; G with -4 gives [7, 11]; E with -5 gives [3, 8].
            vec![-inf, 1.0, 7.0, 5.0, 3.0, 7.0, 3.0, -inf],
            vec![4.0, inf, 7.0, 7.0, 6.0, 11.0, 8.0, inf],
            vec![0.0, -inf, -2.0, 3.5, -inf, 1.0, -1.0],
            vec![4.0, -1.0, inf, 3.5, inf, inf, 2.0],
        )
        .unwrap();
        assert_same(&m.problem, &want);
    }

    #[test]
    fn a_written_problem_reads_back_bit_for_bit() {
        let every = read(EVERY).unwrap().problem;
        let text = write("every", &every).unwrap();
        assert_same(&read(&text).unwrap().problem, &every);

        // Awkward numbers: tenths, the extremes, a subnormal, negative zeros, and two-sided rows whose range reads
        // back exactly as G, exactly only as L, and across a negative and a positive zero.
        let inf = f64::INFINITY;
        let awkward = LpProblem::new(
            vec![0.1, -1e300, 5e-324, -0.0],
            &csr(
                4,
                4,
                &[
                    (0, 0, 0.3),
                    (1, 1, 1e-300),
                    (2, 2, -7.0),
                    (3, 3, 1.0),
                    (0, 3, 2.0),
                ],
            ),
            vec![0.1, -1e16, -0.0, -inf],
            vec![0.3, 0.1, 0.0, inf],
            vec![-0.0, -inf, 0.1, -inf],
            vec![1e300, 2.5, 0.1, -3.0],
        )
        .unwrap();
        let text = write("awkward", &awkward).unwrap();
        assert_same(&read(&text).unwrap().problem, &awkward);
        assert!(text.contains(" L  R1") && text.contains(" N  R3"), "{text}");

        let lossy = LpProblem::new(
            vec![1.0],
            &csr(1, 1, &[(0, 0, 1.0)]),
            vec![1.0],
            vec![9_007_199_254_740_994.0],
            vec![0.0],
            vec![inf],
        )
        .unwrap();
        let e = write("lossy", &lossy).unwrap_err();
        assert!(e.what.contains("no range that reads back exactly"), "{e:?}");
        assert!(write("two words", &every).is_err());
    }

    #[test]
    fn a_small_lp_solved_from_its_mps_text_certifies_its_hand_optimum() {
        // max 3x + 2y s.t. x + y <= 4, x + 3y <= 6, 0 <= x <= 3, 0 <= y <= 5: the vertex (3, 1), worth 11. The rows
        // already hold y to 2; the stated top 5 lets the bound certify (CONTRACT §8, unbounded columns).
        let m = read(
            "NAME SMALL\nOBJSENSE MAX\nROWS\n N obj\n L c1\n L c2\nCOLUMNS\n x obj 3 c1 1\n x c2 1\n y obj 2 c1 1\n \
             y c2 3\nRHS\n rhs c1 4 c2 6\nBOUNDS\n UP bnd x 3\n UP bnd y 5\nENDATA\n",
        )
        .unwrap();
        let s = solve(
            &m.problem,
            &PdlpOptions {
                tolerance: 1e-9,
                max_iterations: 200_000,
                threads: 1,
            },
        )
        .unwrap();
        let bound = s.bound().unwrap().value();
        let best = m.problem.objective(s.x()).unwrap();
        println!(
            r#"{{"test":"mps_small","status":"{:?}","bound":{bound},"objective":{best}}}"#,
            s.status()
        );
        // The bound is a lower bound on the minimisation of -3x - 2y, so at most -11, and close to it.
        assert!(bound <= -11.0 && bound > -11.0 - 1e-6, "{bound}");
        assert!((best + 11.0).abs() < 1e-6, "{best}");
    }

    #[test]
    fn each_defect_is_refused_naming_its_line() {
        const HEAD: &str = "NAME T\nROWS\n N obj\n L c1\n N free\nCOLUMNS\n x obj 1 c1 1\n";
        let ok = format!("{HEAD}RHS\n rhs c1 4\nENDATA\n");
        read(&ok).unwrap();
        refused(
            &format!("{HEAD}FOO\nENDATA\n"),
            "MPS line 8: unknown section \"FOO\"",
        );
        refused(" x obj 1\nENDATA\n", "a data line outside any section");
        refused(HEAD, "there is no ENDATA");
        refused(&format!("{ok}NAME again\n"), "text after ENDATA");
        refused("NAME T\nROWS\n X c1\nENDATA\n", "row type \"X\"");
        refused(
            "NAME T\nROWS\n N obj\n L c1\n G c1\nENDATA\n",
            "a second row called \"c1\"",
        );
        refused(
            "NAME T\nROWS\n N obj\n L obj\nENDATA\n",
            "a second row called \"obj\"",
        );
        refused(
            &format!("{HEAD} x c9 1\nENDATA\n"),
            "no row is called \"c9\"",
        );
        refused(
            &format!("{HEAD} y c1 1\n x c1 2\nENDATA\n"),
            "appears again",
        );
        refused(
            &format!("{HEAD} x c1 2\nENDATA\n"),
            "a second entry in row \"c1\"",
        );
        refused(
            &format!("{HEAD} x obj 2\nENDATA\n"),
            "a second objective entry",
        );
        refused(
            &format!("{HEAD} MARKER 'MARKER' 'INTORG'\nENDATA\n"),
            "integer markers",
        );
        refused(
            &format!("{HEAD} x\nENDATA\n"),
            "a column and one or two row-value pairs",
        );
        refused(
            &format!("{HEAD} x c1 abc\nENDATA\n"),
            "\"abc\" is not a finite number",
        );
        refused(
            &format!("{HEAD} x c1 1e400\nENDATA\n"),
            "\"1e400\" is not a finite number",
        );
        refused(
            &format!("{HEAD}RHS\n rhs c1 4 c1 5\nENDATA\n"),
            "a second right-hand side",
        );
        refused(
            &format!("{HEAD}RHS\n rhs free 4\nENDATA\n"),
            "free row \"free\"",
        );
        refused(
            &format!("{HEAD}RHS\n rhs c1 4\n other obj 1\nENDATA\n"),
            "a second RHS set",
        );
        refused(
            &format!("{HEAD}RANGES\n rng free 4\nENDATA\n"),
            "takes no range",
        );
        refused(
            &format!("{HEAD}RANGES\n rng obj 4\nENDATA\n"),
            "takes no range",
        );
        refused(
            &format!("{HEAD}RANGES\n rng c1 4 c1 5\nENDATA\n"),
            "a second range",
        );
        refused(
            &format!("{HEAD}RHS\n rhs c1 1e308\nRANGES\n rng c1 -1e308\n rng2 c1 1\nENDATA\n"),
            "a second RANGES set",
        );
        refused(
            "NAME T\nROWS\n N obj\n G c1\nCOLUMNS\n x c1 1\nRHS\n rhs c1 1e308\nRANGES\n rng c1 1e308\nENDATA\n",
            "range overflows",
        );
        refused(
            &format!("{HEAD}BOUNDS\n UP bnd x -1\nENDATA\n"),
            "negative upper bound -1",
        );
        refused(
            &format!("{HEAD}BOUNDS\n BV bnd x\nENDATA\n"),
            "integer bound type \"BV\"",
        );
        refused(
            &format!("{HEAD}BOUNDS\n UP bnd y 1\nENDATA\n"),
            "no column is called \"y\"",
        );
        refused(
            &format!("{HEAD}BOUNDS\n UP bnd x 1 2\nENDATA\n"),
            "a BOUNDS line is",
        );
        refused(
            &format!("{HEAD}BOUNDS\n FR bnd x 1\nENDATA\n"),
            "a BOUNDS line is",
        );
        // Three fields of a valued type are a column and a value, so a missing value reads as a bad number.
        refused(
            &format!("{HEAD}BOUNDS\n UP bnd x\nENDATA\n"),
            "\"x\" is not a finite number",
        );
        refused(
            &format!("{HEAD}BOUNDS\n XX bnd x\nENDATA\n"),
            "bound type \"XX\"",
        );
        refused(
            &format!("{HEAD}BOUNDS\n UP a x 1\n UP b x 2\nENDATA\n"),
            "a second BOUNDS set",
        );
        refused(
            "NAME T\nOBJSENSE SIDEWAYS\nENDATA\n",
            "OBJSENSE \"SIDEWAYS\"",
        );
        refused(
            "NAME T\nOBJSENSE\nROWS\nENDATA\n",
            "OBJSENSE has no MIN or MAX",
        );
        refused(
            "NAME T\nROWS\n L c1\nENDATA\n",
            "there is no objective (N) row",
        );
        // LpProblem's own checks pass through with their code.
        let e = read(&format!("{HEAD}BOUNDS\n LO bnd x 2\n UP bnd x 1\nENDATA\n"))
            .map(drop)
            .unwrap_err();
        assert_eq!(e.code, "FMINE-REF-207", "{e:?}");
    }
}
