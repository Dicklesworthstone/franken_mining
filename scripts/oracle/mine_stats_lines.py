# Oracle digests for bd-gwf1.1 (label-anchored Mine Stats reader), computed from W1's recorded read
# (long_all3.json, written by mine_stats_extract3.py before any Rust reader existed). Never generated from Rust output.
#
# One canonical line per parsed data cell, in both views (QTD/YTD summary rows and Minestats blocks):
#   sheet \t cell \t view \t period \t site_raw \t site \t label \t section \t kind \t token
# label   = cleaned metric label (minestats) or the summary field id (summary)
# section = parent row label for child rows (minestats), else "-"
# kind    = number | nil (the dash tokens) | na (n/a) | missing (blank)
# token   = for numbers the IEEE-754 bits as 16 hex digits (exact, no decimal formatting); otherwise the raw token
#           exactly as stored ("— " keeps its trailing space); "" for missing.
# Lines are sorted before hashing, so emission order does not matter.
#
# Usage: python3 -I scripts/oracle/mine_stats_lines.py <long_all3.json> [<dump-dir>]
import collections
import hashlib
import json
import os
import struct
import sys

rows = json.load(open(sys.argv[1], encoding="utf-8"))
dump = sys.argv[2] if len(sys.argv) > 2 else None
lines = collections.defaultdict(list)
counts = collections.defaultdict(collections.Counter)


def kind_token(v):
    if v is None:
        return "missing", ""
    if isinstance(v, bool):
        raise SystemExit(f"boolean cell value {v!r}")
    if isinstance(v, (int, float)):
        return "number", struct.pack(">d", float(v)).hex()
    s = str(v)
    if s.strip() == "—":
        return "nil", s
    if s.strip() == "n/a":
        return "na", s
    raise SystemExit(f"token outside the set: {s!r}")


for r in rows:
    k, tok = kind_token(r["value"])
    label = r["metric"] if r["view"] == "minestats" else r["field"]
    section = r.get("section") or "-"
    line = "\t".join([r["sheet"], r["cell"], r["view"], str(r["period"]), r["site_raw"], r["site"], label, section, k, tok])
    lines[r["file"]].append(line)
    counts[r["file"]][k] += 1
    if k == "number" and float(r["value"]) == 0:
        counts[r["file"]]["number_zero"] += 1

total = 0
for f in sorted(lines, key=lambda x: (x[3:], x[:2])):
    ls = sorted(lines[f])
    digest = hashlib.sha256("".join(l + "\n" for l in ls).encode("utf-8")).hexdigest()
    c = counts[f]
    total += len(ls)
    print(f"{f}\tcells={len(ls)}\tnumber={c['number']}\tzero={c['number_zero']}\tnil={c['nil']}\tna={c['na']}"
          f"\tmissing={c['missing']}\tsha256={digest}")
    if dump:
        with open(os.path.join(dump, f"{f}.oracle.tsv"), "x", encoding="utf-8") as out:
            out.write("".join(l + "\n" for l in ls))
print(f"TOTAL\tcells={total}")
