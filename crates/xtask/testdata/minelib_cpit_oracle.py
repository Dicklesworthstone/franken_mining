"""bd-5lxu: check a MineLib CPIT schedule against the instance's own files, using only the standard library.

Shares no code with fmine-optimize. It reads the `.cpit` and `.prec` text as written, with every decimal kept exact
as a Fraction (no scaling or rounding), and a schedule in MineLib's solution format: one
`<block> <destination> <period> <fraction>` line per mined block, whole blocks only. It checks that each block is
mined at most once, in a period that exists; that every block's required blocks are mined in the same period or
earlier; and that every resource's use in every period is within its `L` limit. The NPV is
`sum over mined blocks of value / (1 + rate)^period`, periods counted from 0, as in
crates/xtask/tests/cpit_schedule.rs, and is computed exactly.

Usage: python3 -I minelib_cpit_oracle.py <name>.cpit <name>.prec <schedule>.sol [...more .sol files]
Prints one JSON line per schedule: {"sol", "feasible", "mined", "npv", "problems"}.
"""

from fractions import Fraction
import json
import sys


def data_lines(text):
    for line in text.splitlines():
        line = line.strip()
        if line and not line.startswith("%"):
            yield line


def read_cpit(text):
    header, sections, current = {}, {}, None
    for line in data_lines(text):
        if line == "EOF":
            break
        key, colon, value = line.partition(":")
        if colon and key.strip().isupper() and not value.strip():
            current = key.strip()
            sections[current] = []
        elif colon and current is None:
            header[key.strip()] = value.strip()
        else:
            sections[current].append(line.split())
    if header["TYPE"] != "CPIT":
        raise ValueError(f"TYPE is {header['TYPE']}")
    n, periods = int(header["NBLOCKS"]), int(header["NPERIODS"])
    resources = int(header["NRESOURCE_SIDE_CONSTRAINTS"])
    limits = {}
    for r, t, kind, v in sections["RESOURCE_CONSTRAINT_LIMITS"]:
        if kind != "L":
            raise ValueError(f"limit kind {kind}")
        limits[(int(r), int(t))] = Fraction(v)
    values = {int(b): Fraction(v) for b, v in sections["OBJECTIVE_FUNCTION"]}
    uses = {(int(b), int(r)): Fraction(q) for b, r, q in sections["RESOURCE_CONSTRAINT_COEFFICIENTS"]}
    if len(values) != n or len(limits) != resources * periods:
        raise ValueError("objective or limit lines missing")
    return n, periods, resources, Fraction(header["DISCOUNT_RATE"]), limits, values, uses


def read_prec(text, n):
    requires = {}
    for line in data_lines(text):
        f = [int(x) for x in line.split()]
        if len(f) != 2 + f[1]:
            raise ValueError(f"precedence line {line!r}")
        requires[f[0]] = f[2:]
    if len(requires) != n:
        raise ValueError("precedence lines missing")
    return requires


def check(instance, requires, sol_text):
    n, periods, resources, rate, limits, values, uses = instance
    problems, period = [], {}
    for line in data_lines(sol_text):
        b, _destination, t, fraction = line.split()
        b, t = int(b), int(t)
        if Fraction(fraction) != 1:
            problems.append(f"block {b} part-mined")
        if b in period:
            problems.append(f"block {b} twice")
        if not (0 <= b < n and 0 <= t < periods):
            problems.append(f"block {b} period {t} out of range")
        period[b] = t
    for b, t in period.items():
        for q in requires.get(b, []):
            if q not in period or period[q] > t:
                problems.append(f"block {b} in {t} needs {q} in {period.get(q)}")
    used = {}
    for b, t in period.items():
        for r in range(resources):
            used[(r, t)] = used.get((r, t), Fraction(0)) + uses.get((b, r), Fraction(0))
    for (r, t), u in sorted(used.items()):
        if u > limits[(r, t)]:
            problems.append(f"resource {r} period {t}: {u} over {limits[(r, t)]}")
    npv = sum((values[b] / (1 + rate) ** t for b, t in period.items()), Fraction(0))
    return {"feasible": not problems, "mined": len(period), "npv": float(npv), "problems": problems[:10]}


def main(argv):
    cpit, prec, *sols = argv
    with open(cpit) as f:
        instance = read_cpit(f.read())
    with open(prec) as f:
        requires = read_prec(f.read(), instance[0])
    for sol in sols:
        with open(sol) as f:
            print(json.dumps({"sol": sol, **check(instance, requires, f.read())}))


if __name__ == "__main__":
    main(sys.argv[1:])
