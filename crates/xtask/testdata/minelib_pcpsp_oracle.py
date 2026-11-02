"""bd-2rne: check a MineLib PCPSP schedule against the instance's own files, using only the standard library.

Shares no code with fmine-optimize. It reads the `.pcpsp` and `.prec` text as written, with every number kept exact
as a Fraction (no scaling or rounding; `-5.65503E+6` included), and a schedule in MineLib's solution format: one
`<block> <destination> <period> <fraction>` line per part of a mined block. A block may be split between
destinations but is mined in one period, so its fractions there must sum to exactly 1. Every block's required
blocks must be mined in the same period or earlier, and every resource's use in every period, summed over the parts,
must be within its `L` limit. The NPV is `sum over parts of fraction * value[block][destination] / (1 + rate)^period`,
periods counted from 0, as in crates/xtask/tests/pcpsp_schedule.rs, and is computed exactly.

MineLib's own PCPSP schedules print fractions to six places, so their sums can miss 1 by a few millionths and their
uses can pass a limit by as much. `worst_whole` (largest |sum - 1| over mined blocks) and `worst_over` (largest use
over a limit, relative to the limit) say by how much, so a reader can tell that rounding from a real violation.

Usage: python3 -I minelib_pcpsp_oracle.py <name>.pcpsp <name>.prec <schedule>.sol [...more .sol files]
Prints one JSON line per schedule: {"sol", "feasible", "mined", "parts", "npv", "worst_whole", "worst_over",
"problem_count", "problems"}.
"""

from fractions import Fraction
import json
import sys


def data_lines(text):
    for line in text.splitlines():
        line = line.strip()
        if line and not line.startswith("%"):
            yield line


def read_pcpsp(text):
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
    if header["TYPE"] != "PCPSP":
        raise ValueError(f"TYPE is {header['TYPE']}")
    if int(header.get("NGENERAL_SIDE_CONSTRAINTS", "0")) != 0:
        raise ValueError("general side constraints are not read")
    n, periods = int(header["NBLOCKS"]), int(header["NPERIODS"])
    destinations = int(header["NDESTINATIONS"])
    resources = int(header["NRESOURCE_SIDE_CONSTRAINTS"])
    limits = {}
    for r, t, kind, v in sections["RESOURCE_CONSTRAINT_LIMITS"]:
        if kind != "L":
            raise ValueError(f"limit kind {kind}")
        limits[(int(r), int(t))] = Fraction(v)
    values = {}
    for b, *v in sections["OBJECTIVE_FUNCTION"]:
        if len(v) != destinations:
            raise ValueError(f"block {b}: {len(v)} values for {destinations} destinations")
        values[int(b)] = [Fraction(x) for x in v]
    uses = {}
    for b, d, r, q in sections["RESOURCE_CONSTRAINT_COEFFICIENTS"]:
        uses.setdefault((int(b), int(d)), []).append((int(r), Fraction(q)))
    if len(values) != n or len(limits) != resources * periods:
        raise ValueError("objective or limit lines missing")
    return n, periods, destinations, Fraction(header["DISCOUNT_RATE"]), limits, values, uses


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
    n, periods, destinations, rate, limits, values, uses = instance
    problems, period, share, used, parts = [], {}, {}, {}, 0
    npv = Fraction(0)
    for line in data_lines(sol_text):
        b, d, t, y = line.split()
        b, d, t, y = int(b), int(d), int(t), Fraction(y)
        parts += 1
        if not (0 <= b < n and 0 <= d < destinations and 0 <= t < periods):
            problems.append(f"block {b} destination {d} period {t} out of range")
            continue
        if not 0 <= y <= 1:
            problems.append(f"block {b} fraction {y} outside [0, 1]")
        if period.setdefault(b, t) != t:
            problems.append(f"block {b} in periods {period[b]} and {t}")
        share[b] = share.get(b, Fraction(0)) + y
        npv += y * values[b][d] / (1 + rate) ** t
        for r, q in uses.get((b, d), []):
            used[(r, t)] = used.get((r, t), Fraction(0)) + y * q
    worst_whole = Fraction(0)
    for b, s in share.items():
        worst_whole = max(worst_whole, abs(s - 1))
        if s != 1:
            problems.append(f"block {b} fractions sum to {s}")
    for b, t in period.items():
        for q in requires.get(b, []):
            if q not in period or period[q] > t:
                problems.append(f"block {b} in {t} needs {q} in {period.get(q)}")
    worst_over = Fraction(0)
    for (r, t), u in sorted(used.items()):
        cap = limits[(r, t)]
        if u > cap:
            problems.append(f"resource {r} period {t}: {u} over {cap}")
            worst_over = max(worst_over, (u - cap) / max(abs(cap), Fraction(1)))
    return {
        "feasible": not problems,
        "mined": len(period),
        "parts": parts,
        "npv": float(npv),
        "worst_whole": float(worst_whole),
        "worst_over": float(worst_over),
        "problem_count": len(problems),
        "problems": problems[:10],
    }


def main(argv):
    pcpsp, prec, *sols = argv
    with open(pcpsp) as f:
        instance = read_pcpsp(f.read())
    with open(prec) as f:
        requires = read_prec(f.read(), instance[0])
    for sol in sols:
        with open(sol) as f:
            print(json.dumps({"sol": sol, **check(instance, requires, f.read())}))


if __name__ == "__main__":
    main(sys.argv[1:])
