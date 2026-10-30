"""Share census: the BD-331.S3 "stated share rule" applied to every gold cost-of-sales-per-ounce instance that
census2.py refuses with "attributable share needed" (a non-zero non-controlling-interest line in its column).

Independent second reader, written 2026-10-30 without opening any Rust under crates/ and without reading
rate_census.py. It imports census2.py, census.py and xl.py (written before the Rust) only to enumerate the
in-scope instances; every row, footnote, label and share it uses is located here, by text.

Rule, as written in BD-331.S3 (applied as stated, not tuned):
1. Footnote: on the table's own sheet, a cell whose text reads "Gold cost of sales per ounce is calculated as
   cost of sales ... divided by ounces sold (both on an attributable basis using Barrick's ownership share)".
   Judgement call: the 2026 workbooks abbreviate the defined term as "Gold COS/oz is calculated as cost of
   sales ..."; the rate row on those sheets is labelled "COS/oz", so the abbreviation is accepted as the same
   footnote. No footnote on the sheet -> the rule is not applied (counted in no_footnote).
2. Registered shares (exact rationals): REGISTERED below.
3. Label: in the same-release Mine Statistics workbook (same registry quarter as the AISC file), on its
   quarter-to-date sheet, a column-A label "<name> (<p>%)" with any trailing footnote digits stripped; the
   printed p% must equal the registered share exactly. The quarter-to-date sheet is chosen from content: the
   unique visible sheet whose A1 reads "Summary of Operations" and whose column-A period line reads "For the
   three months ended <Month> <day>,"; the month must be the registry quarter's last month and the first
   numeric cell on the period row must be the registry quarter's year. Missing or mismatched label -> (v).
4. Check (site with confirmed share s = a/b; n cost of sales $M, q ounces sold 000s, r cost of sales per
   ounce, each printed integer x standing for [x - 1/2, x + 1/2]): fit iff the corner range of
   1000*a*(2n +- 1) / (b*(2q +- 1)) meets [r - 1/2, r + 1/2] -> (i); else (iv) with the feasible share interval
   [(r - 1/2)(q - 1/2) / (1000(n + 1/2)), (r + 1/2)(q + 1/2) / (1000(n - 1/2))] rounded to 4 decimals.
5. Regions (segment roll-ups whose members carry different ownership shares) stay (v): "regional numerator
   needs member weighting".
6. Sites with no registered share are reported separately (state stays v; share null).
Order of tests per instance: footnote, region, registered share, label, check.

Output: one JSON line per in-scope instance
  {"file","sheet","cell","subject","share","state","s_lo","s_hi","footnote_cell","label_cell"}
(cell: the rate cell; footnote_cell: cell on the same sheet; label_cell: "<sheet>!<cell>" in the same-release
Mine Statistics workbook, "not found" when the label is absent, null when no label was sought), then one
summary line {"sites","fit","miss","unconfirmed","regions_v","no_footnote","no_registered_share"}.
usage: python3 -I scripts/oracle/share_census.py <workspace_root>
"""
import html
import json
import os
import re
import sys
import tomllib
from fractions import Fraction as F

sys.dont_write_bytecode = True  # leave no __pycache__ entries behind in the repository
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xl import load  # noqa: E402
import census2  # noqa: E402

REGISTERED = {
    "Carlin": F(123, 200), "Cortez": F(123, 200), "Turquoise Ridge": F(123, 200), "Phoenix": F(123, 200),
    "Nevada Gold Mines": F(123, 200),
    "Pueblo Viejo": F(3, 5),
    "North Mara": F(21, 25), "Bulyanhulu": F(21, 25),
    "Loulo-Gounkoto": F(4, 5),
    "Tongon": F(897, 1000),
}
# Mine Statistics label for a subject, where it differs from the subject name.
MS_LABEL = {"Nevada Gold Mines": "Nevada Gold Mines LLC"}
# Operating-segment roll-ups: each sums sites held at different ownership shares.
REGIONS = {"North America", "Latin America & Asia Pacific", "Africa & Middle East"}
QUARTER_END_MONTH = {1: "march", 2: "june", 3: "september", 4: "december"}

FOOTNOTE = re.compile(
    r"gold (?:cost of sales per ounce|cos/oz) is calculated as cost of sales\b.*"
    r"\bdivided by ounces sold \(both on an attributable basis using barrick['’]s ownership share\)")


def norm(s):
    return " ".join(html.unescape(str(s)).split())


def rc_key(ref):
    m = re.fullmatch(r"([A-Z]+)([0-9]+)", ref)
    return int(m.group(2)), len(m.group(1)), m.group(1)


def column_a(cells):
    return sorted((int(r[1:]), norm(v)) for r, v in cells.items() if re.fullmatch(r"A[0-9]+", r))


def registry(root):
    with open(os.path.join(root, "registries/sources/barrick.toml"), "rb") as fh:
        reg = tomllib.load(fh)
    quarter = {s["file"]: s["quarter"] for s in reg["source"]}
    mine_stats = {}
    for s in reg["source"]:
        if s["kind"] == "mine_stats":
            if s["quarter"] in mine_stats:
                raise SystemExit("two Mine Statistics files for quarter %s" % s["quarter"])
            mine_stats[s["quarter"]] = s["file"]
    return os.path.join(root, reg["corpus_dir"]), quarter, mine_stats


def qtd_sheet(path, quarter):
    """The quarter-to-date sheet of a Mine Statistics workbook, chosen from content (see module docstring)."""
    year, qn = int(quarter[:4]), int(quarter[-1])
    cells = load(path)
    found = []
    for sh in census2.visible(path):
        c = cells[sh]
        a = column_a(c)
        if not a or a[0][1].lower() != "summary of operations":
            continue
        period = next(((r, t) for r, t in a if re.match(r"for the \w+ months ended ", t.lower())), None)
        if period is None:
            continue
        m = re.fullmatch(r"for the (\w+) months ended (\w+) [0-9]+,?", period[1].lower())
        if not m or m.group(1) != "three":
            continue
        row = period[0]
        nums = [(rc_key(ref), v) for ref, v in c.items()
                if rc_key(ref)[0] == row and rc_key(ref)[2] != "A" and re.fullmatch(r"[0-9]+(\.0+)?", norm(v))]
        first_year = int(float(norm(min(nums)[1]))) if nums else None
        if m.group(2) != QUARTER_END_MONTH[qn] or first_year != year:
            raise SystemExit("%s: three-month sheet %r reads %r, year %r; registry quarter is %s"
                             % (os.path.basename(path), sh, period[1], first_year, quarter))
        found.append((sh, "A%d" % row, period[1]))
    if len(found) != 1:
        raise SystemExit("%s: expected one quarter-to-date sheet, found %r" % (os.path.basename(path), found))
    sh = found[0][0]
    return sh, cells[sh], found[0]


def find_label(qcells, subject):
    """[(cell, printed share or None)] for column-A labels naming the subject (trailing digits stripped)."""
    want = MS_LABEL.get(subject, subject)
    hits = []
    for row, text in column_a(qcells):
        t = re.sub(r"[0-9]+$", "", text).rstrip()
        m = re.fullmatch(r"(.+?)\s*\(([0-9]+(?:\.[0-9]+)?)%\)", t)
        name, pct = (m.group(1), F(m.group(2)) / 100) if m else (t, None)
        if name == want:
            hits.append(("A%d" % row, pct))
    return hits


def sheet_layout(cells):
    """Rows (by column-A text) of the cost-of-sales line, ounces sold and cost of sales per ounce, and the
    footnote cell (any column) defining gold cost of sales per ounce."""
    lab = {r: t.lower() for r, t in column_a(cells)}
    one = lambda *names: [r for r, t in lab.items() if t in names]  # noqa: E731
    rc = one("cost of sales applicable to gold production")
    ro = [r for r, t in lab.items() if t.startswith("ounces sold")]
    rr = one("cost of sales per ounce", "cos/oz")
    if len(rc) != 1 or len(ro) != 1 or len(rr) != 1:
        raise SystemExit("ambiguous AISC layout: cos %r ounces %r rate %r" % (rc, ro, rr))
    notes = sorted((rc_key(ref), ref) for ref, v in cells.items() if FOOTNOTE.search(norm(v).lower()))
    if len(notes) > 1:
        raise SystemExit("more than one cost-of-sales-per-ounce footnote: %r" % [n[1] for n in notes])
    return rc[0], ro[0], rr[0], (notes[0][1] if notes else None)


def integer(cells, ref):
    v = census2.val(cells, ref)
    if v[0] != "num" or v[1] != int(v[1]):
        raise SystemExit("%s is not a printed integer: %r" % (ref, v))
    return int(v[1])


def check(share, n, q, r):
    """(fit, s_lo, s_hi): exact corner test and the exact feasible share interval."""
    if n < 1 or q < 1 or r < 1:
        raise SystemExit("non-positive operand n=%d q=%d r=%d" % (n, q, r))
    a, b = share.numerator, share.denominator
    corners = [F(1000 * a * (2 * n + i), b * (2 * q + j)) for i in (-1, 1) for j in (-1, 1)]
    fit = max(corners) >= F(2 * r - 1, 2) and min(corners) <= F(2 * r + 1, 2)
    s_lo = F((2 * r - 1) * (2 * q - 1), 2000 * (2 * n + 1))
    s_hi = F((2 * r + 1) * (2 * q + 1), 2000 * (2 * n - 1))
    if fit != (s_lo <= share <= s_hi):
        raise SystemExit("corner test and share interval disagree at n=%d q=%d r=%d" % (n, q, r))
    return fit, s_lo, s_hi


def dec4(x):
    return float(round(x, 4))


def run(root):
    corpus, quarter, mine_stats = registry(root)
    _, res = census2.run(root)
    scope = [x for x in res if x[4] == "COS/oz" and x[5] == "v" and x[6] == "attributable share needed"]
    aisc, qtd, out = {}, {}, []
    for fn, sh, col, subj, _fam, _state, _why in scope:
        if fn not in aisc:
            aisc[fn] = load(os.path.join(corpus, fn))
        cells = aisc[fn][sh]
        rc, ro, rr, note = sheet_layout(cells)
        rec = {"file": fn, "sheet": sh, "cell": "%s%d" % (col, rr), "subject": subj, "share": None,
               "state": "v", "s_lo": None, "s_hi": None, "footnote_cell": note, "label_cell": None}
        kind = None
        if note is None:
            kind = "no_footnote"
        elif subj in REGIONS:
            kind = "regions_v"
        elif subj not in REGISTERED:
            kind = "no_registered_share"
        else:
            share = REGISTERED[subj]
            rec["share"] = "%d/%d" % (share.numerator, share.denominator)
            q = quarter[fn]
            if q not in qtd:
                ms = mine_stats[q]
                qsh, qcells, _ = qtd_sheet(os.path.join(corpus, ms), q)
                qtd[q] = (qsh, qcells)
            qsh, qcells = qtd[q]
            hits = find_label(qcells, subj)
            if len(hits) != 1:
                rec["label_cell"] = "not found" if not hits else ",".join("%s!%s" % (qsh, h[0]) for h in hits)
                kind = "unconfirmed"
            else:
                rec["label_cell"] = "%s!%s" % (qsh, hits[0][0])
                if hits[0][1] != share:
                    kind = "unconfirmed"
                else:
                    n = integer(cells, "%s%d" % (col, rc))
                    oz = integer(cells, "%s%d" % (col, ro))
                    r = integer(cells, "%s%d" % (col, rr))
                    fit, s_lo, s_hi = check(share, n, oz, r)
                    if fit:
                        kind, rec["state"] = "fit", "i"
                    else:
                        kind, rec["state"] = "miss", "iv"
                        rec["s_lo"], rec["s_hi"] = dec4(s_lo), dec4(s_hi)
        out.append((kind, rec))
    return out


if __name__ == "__main__":
    rows = run(sys.argv[1])
    summary = dict.fromkeys(("sites", "fit", "miss", "unconfirmed", "regions_v", "no_footnote",
                             "no_registered_share"), 0)
    for kind, rec in rows:
        print(json.dumps(rec))
        summary[kind] += 1
        if rec["subject"] not in REGIONS:
            summary["sites"] += 1
    print(json.dumps(summary))
