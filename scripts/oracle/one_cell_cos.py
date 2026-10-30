"""K.4's cheapest common test (bd-prmn.8): the one-cell cost-of-sales-per-ounce check, share-scaled where the
column mixes bases, on every registered-subject column of every visible AISC table.

Written 2026-10-30, before the Rust diagnostic, by the agent that then wrote it (same origin: agreement between
the two is one reader of one stated rule, not two). It imports census2.py (table layout helpers) and
share_census.py (registered shares, regions, footnote and label confirmation), both written before the Rust they
check, and adds only the rule below.

Rule (DESIGN, stated before the Rust):
- Scope: each column of a visible AISC table whose header is a registered subject (census2's layout: column-A
  labels, the header row above the cost-of-sales row with the most registered subjects, "Footnote" skipped).
- Operands: n cost of sales ($M, the cost-of-sales row), q ounces sold (000s), r cost of sales per ounce, each
  printed integer x standing for [x - 1/2, x + 1/2]. NCI: a "non-controlling interests" line in either block of
  the column holds a non-zero number.
- Refused, in this order: all used cells of the column zero or placeholder ("all-zero column"); r not a number
  ("rate not published"); q not a number ("ounces not numeric"); r or q zero ("zero rate or ounces"); n not a
  number ("cost of sales not numeric"); n, q or r not an integer ("unrounded stored value"); n, q or r below 1
  ("non-positive operand").
- Basis: 100% when the column has no NCI. With NCI: a region -> refused ("regional numerator needs member
  weighting"); no attributable-basis footnote on the sheet -> refused ("attributable-basis footnote not found");
  no registered share -> refused ("no registered share"); the same-release Mine Statistics label not printing
  the registered share -> refused ("registered share not confirmed by label"); else the registered share s.
- Check at share s = a/b: pass iff the corner range of 1000*a*(2n +- 1) / (b*(2q +- 1)) meets [r - 1/2, r + 1/2].
  at_full is the check at s = 1; at_share the check at the subject's registered share (sites with one only,
  whatever the column's basis), so a column that fails at its basis and passes at the other is visible.
- Implied ounces at the basis: Q in [1000*a*(2n - 1) / (b*(2r + 1)), 1000*a*(2n + 1) / (b*(2r - 1))], the ounces
  that would close the check for some cost of sales in [n - 1/2, n + 1/2]; printed to 3 places, rounded outward.
  pass iff [q - 1/2, q + 1/2] meets that interval (asserted equal to the corner test).
- K.4 items (Appendix K.4 "Still open"; E691, E817, E819): Veladero, periods in 2024 or 2025; Hemlo, the sheets
  "Q4 2025" and "2025"; North Mara, the sheets "Q2 2026" and "YTD Q2 2026"; Latin America & Asia Pacific,
  periods in 2023. A sheet's period year is the 4-digit number in its name.
- Verdict: the failing columns outside the K.4 items; "site-specific" if there are none, otherwise "basis rule"
  with the failing columns grouped by (subject, basis, passes at the other basis). All failing columns, K.4
  items included, are also counted by sheet name, so a period shared across subjects shows.

Output: one JSON line per column {"file","sheet","column","subject","kind","basis","state","reason","at_full",
"at_share","implied_oz_lo","implied_oz_hi","k4_item"} (state "pass", "fail" or "refused"), then one summary line
{"columns","pass","fail","refused","k4_fail","other_fail","verdict","pattern","fail_by_sheet"}. --quiet prints the
summary only.
usage: python3 -I scripts/oracle/one_cell_cos.py [--quiet] <workspace_root>
"""
import glob
import json
import os
import re
import sys
from fractions import Fraction as F

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import census2  # noqa: E402
import share_census as sc  # noqa: E402
from census import colname  # noqa: E402
from xl import load  # noqa: E402

K4 = {
    "Veladero": lambda sheet: year(sheet) in (2024, 2025),
    "Hemlo": lambda sheet: sheet in ("Q4 2025", "2025"),
    "North Mara": lambda sheet: sheet in ("Q2 2026", "YTD Q2 2026"),
    "Latin America & Asia Pacific": lambda sheet: year(sheet) == 2023,
}


def year(sheet):
    m = re.search(r"\b(20[0-9]{2})\b", sheet)
    return int(m.group(1)) if m else None


def fits(a, b, n, q, r):
    corners = [F(1000 * a * (2 * n + i), b * (2 * q + j)) for i in (-1, 1) for j in (-1, 1)]
    return max(corners) >= F(2 * r - 1, 2) and min(corners) <= F(2 * r + 1, 2)


def three(x, up):
    t = x * 1000
    t = -((-t.numerator) // t.denominator) if up else t.numerator // t.denominator
    return "%d.%03d" % (t // 1000, t % 1000)


def confirmed(root, fn, subj, cache):
    """Whether the same-release Mine Statistics label prints the registered share (share_census's rule)."""
    corpus, quarter, mine_stats = cache.setdefault("registry", sc.registry(root))
    q = quarter[fn]
    if q not in cache:
        qsh, qcells, _ = sc.qtd_sheet(os.path.join(corpus, mine_stats[q]), q)
        cache[q] = qcells
    hits = sc.find_label(cache[q], subj)
    return len(hits) == 1 and hits[0][1] == sc.REGISTERED[subj]


def run(root):
    d = os.path.join(root, "corpus_of_input_docs/downloaded/mine_stats")
    out, cache = [], {}
    for f in sorted(glob.glob(os.path.join(d, "*AISC.xlsx"))):
        fn = os.path.basename(f)
        allc = load(f)
        for sh in census2.visible(f):
            c = allc[sh]
            lab = {r: census2.norm(c["A%d" % r]).lower() for r in range(1, 200) if c.get("A%d" % r) is not None}
            one = lambda *n: [r for r, l in lab.items() if l in n]  # noqa: E731
            rc = one("cost of sales applicable to gold production")
            if not rc:
                continue
            rc, rt, ra = rc[0], one("total cash costs")[0], one("all-in sustaining costs")[0]
            ro = [r for r, l in lab.items() if l.startswith("ounces sold")][0]
            rr = one("cost of sales per ounce", "cos/oz")[0]
            rates = [rr, one("total cash costs per ounce", "tcc/oz")[0],
                     one("all-in sustaining costs per ounce", "aisc/oz")[0]]
            b1 = [r for r in sorted(lab) if rc <= r < rt]
            b2 = [r for r in sorted(lab) if rt < r < ra]
            nci = [r for r in b1 + b2 if lab[r] == "non-controlling interests"]
            footnote = sc.sheet_layout(c)[3]
            best, hr = -1, None
            for r in range(1, rc):
                k = sum(1 for i in range(2, 60) if census2.subject(c.get(colname(i) + str(r), "")))
                if k > best:
                    best, hr = k, r
            for i in range(2, 60):
                col = colname(i)
                h = c.get(f"{col}{hr}")
                if h is None or census2.norm(h).lower() == "footnote":
                    continue
                subj = census2.subject(h)
                if subj is None:
                    continue
                rec = {"file": fn, "sheet": sh, "column": col, "subject": subj,
                       "kind": "region" if subj in sc.REGIONS else "site", "basis": None, "state": "refused",
                       "reason": None, "at_full": None, "at_share": None, "implied_oz_lo": None,
                       "implied_oz_hi": None, "k4_item": subj in K4 and K4[subj](sh)}
                out.append(rec)
                vs = {r: census2.val(c, f"{col}{r}") for r in b1 + [rt] + b2 + [ra, ro] + rates}
                if all(v[0] == "place" or (v[0] == "num" and v[1] == 0) for v in vs.values()):
                    rec["reason"] = "all-zero column"
                    continue
                vn, vq, vr = vs[rc], vs[ro], vs[rr]
                if vr[0] != "num":
                    rec["reason"] = "rate not published"
                elif vq[0] != "num":
                    rec["reason"] = "ounces not numeric"
                elif vr[1] == 0 or vq[1] == 0:
                    rec["reason"] = "zero rate or ounces"
                elif vn[0] != "num":
                    rec["reason"] = "cost of sales not numeric"
                elif any(v[1] != int(v[1]) for v in (vn, vq, vr)):
                    rec["reason"] = "unrounded stored value"
                elif any(v[1] < 1 for v in (vn, vq, vr)):
                    rec["reason"] = "non-positive operand"
                if rec["reason"]:
                    continue
                n, q, r = int(vn[1]), int(vq[1]), int(vr[1])
                rec["at_full"] = fits(1, 1, n, q, r)
                site_share = sc.REGISTERED.get(subj) if subj not in sc.REGIONS else None
                if site_share is not None:
                    rec["at_share"] = fits(site_share.numerator, site_share.denominator, n, q, r)
                if any(vs[x][0] == "num" and vs[x][1] != 0 for x in nci):
                    if subj in sc.REGIONS:
                        rec["reason"] = "regional numerator needs member weighting"
                    elif footnote is None:
                        rec["reason"] = "attributable-basis footnote not found"
                    elif site_share is None:
                        rec["reason"] = "no registered share"
                    elif not confirmed(root, fn, subj, cache):
                        rec["reason"] = "registered share not confirmed by label"
                    if rec["reason"]:
                        continue
                    s = site_share
                else:
                    s = F(1)
                a, b = s.numerator, s.denominator
                rec["basis"] = "%d/%d" % (a, b)
                lo, hi = F(1000 * a * (2 * n - 1), b * (2 * r + 1)), F(1000 * a * (2 * n + 1), b * (2 * r - 1))
                rec["implied_oz_lo"], rec["implied_oz_hi"] = three(lo, False), three(hi, True)
                ok = fits(a, b, n, q, r)
                if ok != (F(2 * q + 1, 2) >= lo and F(2 * q - 1, 2) <= hi):
                    raise SystemExit("corner test and implied ounces disagree: %s %s %s" % (fn, sh, col))
                rec["state"] = "pass" if ok else "fail"
    return out


def summary(rows):
    fails = [x for x in rows if x["state"] == "fail"]
    other = [x for x in fails if not x["k4_item"]]
    groups = {}
    for x in other:
        other_basis = x["at_share"] if x["basis"] == "1/1" else x["at_full"]
        key = (x["subject"], x["basis"], other_basis)
        groups[key] = groups.get(key, 0) + 1
    pattern = [{"subject": s, "basis": b, "passes_at_other_basis": o, "columns": n}
               for (s, b, o), n in sorted(groups.items(), key=lambda kv: (-kv[1], kv[0][0]))]
    by_sheet = {}
    for x in fails:
        by_sheet[x["sheet"]] = by_sheet.get(x["sheet"], 0) + 1
    return {"columns": len(rows), "pass": sum(x["state"] == "pass" for x in rows), "fail": len(fails),
            "refused": sum(x["state"] == "refused" for x in rows), "k4_fail": len(fails) - len(other),
            "other_fail": len(other), "verdict": "site-specific" if not other else "basis rule",
            "pattern": pattern, "fail_by_sheet": dict(sorted(by_sheet.items(), key=lambda kv: (-kv[1], kv[0])))}


if __name__ == "__main__":
    args = sys.argv[1:]
    quiet = args[:1] == ["--quiet"]
    rows = run(args[-1])
    if not quiet:
        for x in rows:
            print(json.dumps(x, ensure_ascii=False))
    print(json.dumps(summary(rows), ensure_ascii=False))
