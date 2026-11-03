"""bd-06kg: solve an LP written by `fmine_lp::mps::write` with HiGHS, as an outside solver (plan §15.4).

Shares no code with fmine-lp. It reads free-format MPS, the subset that `write` emits: `NAME`, `ROWS` (`N`, `L`,
`G`, `E`), `COLUMNS`, `RHS`, `RANGES` and `BOUNDS` (`UP`, `LO`, `FX`, `FR`, `MI`, `PL`), with a column's default
bounds `[0, +inf)`. Anything else is refused. A range `R` on a row with right-hand side `b` gives `[b, b + |R|]` on
a `G` row and `[b - |R|, b]` on an `L` row; on an `E` row, `[b, b + R]` for `R > 0` and `[b + R, b]` for `R < 0`.
It minimises with `scipy.optimize.linprog(method="highs")`, each row split into `<=` rows (an `E` row is an
equality), and prints one solution in `fmine_lp::mps::read_values`'s format:

- `--columns`: `<column> <x_j>` for every column, under the file's names;
- `--rows`: `<row> <y_i>` for every row but the objective, under the file's names. `y_i` is the objective's
  derivative with respect to the row's bounds: HiGHS's marginal on the `<=` row its upper bound gives, minus its
  marginal on the negated row its lower bound gives, and the equality marginal for an `E` row. That is
  `certified_bound`'s sign: `y_i < 0` on a row held at its upper bound. A later `N` row is a free row, with `y_i = 0`.

Values are printed in Python's `repr`, which reads back exactly. A first `#` line gives the scipy version, HiGHS's
status and the objective. It exits 1, printing nothing on stdout, unless HiGHS reports the LP solved to optimality.

Usage: python3 -I scripts/oracle/highs_lp.py <file>.mps (--columns | --rows)
"""

import math
import sys

import numpy as np
import scipy
from scipy.optimize import linprog


def refuse(line, detail):
    sys.exit(f"MPS line {line}: {detail}")


def number(line, text):
    try:
        v = float(text)
    except ValueError:
        refuse(line, f"{text!r} is not a number")
    if not math.isfinite(v):
        refuse(line, f"{text!r} is not finite")
    return v


def read_mps(text):
    rows, kinds, objective = [], {}, None
    columns, col_of = [], {}
    cost, entries, rhs, ranges = {}, {}, {}, {}
    lower, upper = {}, {}
    section = None
    for k, raw in enumerate(text.splitlines(), start=1):
        if not raw.strip() or raw.startswith("*"):
            continue
        fields = raw.split()
        if not raw[0].isspace():
            section = fields[0]
            if section == "ENDATA":
                break
            if section not in ("NAME", "ROWS", "COLUMNS", "RHS", "RANGES", "BOUNDS"):
                refuse(k, f"section {section!r} is not read here")
            continue
        if section == "ROWS":
            kind, name = fields
            if kind == "N" and objective is None:
                objective = name
            elif kind in ("N", "L", "G", "E"):
                kinds[name] = kind
                rows.append(name)
            else:
                refuse(k, f"row type {kind!r}")
        elif section == "COLUMNS":
            name, pairs = fields[0], fields[1:]
            if name not in col_of:
                col_of[name] = len(columns)
                columns.append(name)
            for r, v in zip(pairs[0::2], pairs[1::2]):
                if r == objective:
                    cost[name] = number(k, v)
                elif r in kinds:
                    entries[(r, name)] = number(k, v)
                else:
                    refuse(k, f"no row {r!r}")
        elif section in ("RHS", "RANGES"):
            pairs = fields[1:] if len(fields) % 2 == 1 else fields
            for r, v in zip(pairs[0::2], pairs[1::2]):
                if r not in kinds:
                    refuse(k, f"{section} on {r!r}")
                (rhs if section == "RHS" else ranges)[r] = number(k, v)
        elif section == "BOUNDS":
            kind, name = fields[0], fields[2]
            if name not in col_of:
                refuse(k, f"no column {name!r}")
            if kind == "UP":
                upper[name] = number(k, fields[3])
            elif kind == "LO":
                lower[name] = number(k, fields[3])
            elif kind == "FX":
                lower[name] = upper[name] = number(k, fields[3])
            elif kind == "FR":
                lower[name], upper[name] = -math.inf, math.inf
            elif kind == "MI":
                lower[name] = -math.inf
            elif kind == "PL":
                upper[name] = math.inf
            else:
                refuse(k, f"bound type {kind!r}")
        else:
            refuse(k, "a data line outside ROWS, COLUMNS, RHS, RANGES or BOUNDS")
    else:
        refuse(len(text.splitlines()), "there is no ENDATA")
    row_lo, row_hi = [], []
    for r in rows:
        b, rg, kind = rhs.get(r, 0.0), ranges.get(r), kinds[r]
        if kind == "N":
            lo, hi = -math.inf, math.inf
        elif kind == "L":
            lo, hi = (-math.inf if rg is None else b - abs(rg)), b
        elif kind == "G":
            lo, hi = b, (math.inf if rg is None else b + abs(rg))
        elif rg is None:
            lo, hi = b, b
        else:
            lo, hi = (b, b + rg) if rg > 0 else (b + rg, b)
        row_lo.append(lo)
        row_hi.append(hi)
    a = np.zeros((len(rows), len(columns)))
    row_of = {r: i for i, r in enumerate(rows)}
    for (r, c), v in entries.items():
        a[row_of[r], col_of[c]] = v
    c = np.array([cost.get(name, 0.0) for name in columns])
    bounds = [
        (lower.get(name, 0.0), upper.get(name, math.inf)) for name in columns
    ]
    return rows, columns, a, c, row_lo, row_hi, bounds


def main(argv):
    if len(argv) != 3 or argv[2] not in ("--columns", "--rows"):
        sys.exit(__doc__.strip().splitlines()[-1])
    with open(argv[1], encoding="ascii") as f:
        rows, columns, a, c, row_lo, row_hi, bounds = read_mps(f.read())
    eq = [i for i in range(len(rows)) if row_lo[i] == row_hi[i]]
    up = [i for i in range(len(rows)) if row_lo[i] != row_hi[i] and math.isfinite(row_hi[i])]
    down = [i for i in range(len(rows)) if row_lo[i] != row_hi[i] and math.isfinite(row_lo[i])]
    a_ub = np.vstack([a[up], -a[down]]) if up or down else None
    b_ub = np.array([row_hi[i] for i in up] + [-row_lo[i] for i in down]) if up or down else None
    res = linprog(
        c,
        A_ub=a_ub,
        b_ub=b_ub,
        A_eq=a[eq] if eq else None,
        b_eq=np.array([row_lo[i] for i in eq]) if eq else None,
        bounds=bounds,
        method="highs",
    )
    if res.status != 0:
        sys.exit(f"HiGHS status {res.status}: {res.message}")
    print(f"# scipy {scipy.__version__} linprog(method=\"highs\"): {res.message} objective {res.fun!r}")
    if argv[2] == "--columns":
        for j, v in enumerate(res.x):
            print(f"{columns[j]} {float(v)!r}")
        return
    y = [0.0] * len(rows)
    marginals = list(res.ineqlin.marginals) if (up or down) else []
    for k, i in enumerate(up):
        y[i] += float(marginals[k])
    for k, i in enumerate(down):
        y[i] -= float(marginals[len(up) + k])
    for k, i in enumerate(eq):
        y[i] += float(res.eqlin.marginals[k])
    for i, v in enumerate(y):
        print(f"{rows[i]} {v!r}")


if __name__ == "__main__":
    main(sys.argv)
