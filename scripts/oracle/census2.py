"""Independent census v2 of the AISC identity ledger: the reference read for fmine-public's identity module.

Rules (DESIGN, stated before the Rust run):
- visible sheets only; a sheet is an AISC table when column A holds the cost-of-sales label
- header row: the row above the cost-of-sales row with the most registered subjects; 'Footnote' ignored;
  a header is a registered subject, optionally followed by one lowercase footnote letter
- used cells of a column: block-1 lines (COS row up to TCC row), TCC, block-2 lines, AISC, ounces, three rates
- a column whose used cells are all numeric zero or placeholder is NOT_PUBLISHED (all its instances)
- footing (component-sum, exact): TCC = sum(block 1); AISC = TCC + sum(block 2)
  total not numeric -> not published; any used cell blank/text/error -> refused; any non-integer -> refused;
  equal -> consistent (i); unequal -> issuer discrepancy (iii)
- per ounce: printed rate r vs 1000 N / Q on intervals N +- k/2 (k: COS 1, TCC numeric lines of block 1,
  AISC numeric lines of both blocks), Q +- 1/2, r +- 1/2; exact rational comparison
  rate not numeric -> not published; rate 0 and Q 0 -> not published; one of them 0 -> refused;
  non-integer -> refused; COS with a non-zero non-controlling-interest line -> refused;
  inside -> consistent (i); outside -> undecidable (iv), or explained (ii) when a registered reconstruction closes
- alternate construction (subtotal rounded once) is reported, not scored
"""
import glob, html, os, re, sys, zipfile
from fractions import Fraction as F
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xl import load
from census import SUBJECTS, colname

PLACE = {"-", "–", "—", "N/A", "n/a", "NA", "na"}


def norm(s):
    return " ".join(html.unescape(str(s)).split())


def subject(h):
    h = norm(h)
    for n in SUBJECTS:
        if h == n or (h.startswith(n) and len(h) == len(n) + 1 and h[-1].islower()):
            return n
    return None


def visible(path):
    wb = zipfile.ZipFile(path).read("xl/workbook.xml").decode()
    out = []
    for t in re.findall(r"<sheet\b[^>]*>", wb):
        st = re.search(r'\bstate="([^"]*)"', t)
        if not (st and st.group(1) in ("hidden", "veryHidden")):
            out.append(html.unescape(re.search(r'\bname="([^"]*)"', t).group(1)))
    return out


def val(c, ref):
    v = c.get(ref)
    if v is None:
        return ("blank", None)
    s = norm(v).strip()
    if s in PLACE:
        return ("place", None)
    try:
        return ("num", float(s))
    except ValueError:
        return ("text", s)


# registered reconstructions (E816): (aisc file, sheet, subject, identity) -> (mine stats file, sheet, cell)
RECON = {
    ("Barrick_Q1_2025_AISC.xlsx", "Q1 2025", "Africa & Middle East", "AISC"):
        ("Barrick_Q1_2025_Mine_Stats.xlsx", "Minestats3", "B32"),
    ("Barrick_Q2_2025_AISC.xlsx", "Q1 2025", "Africa & Middle East", "AISC"):
        ("Barrick_Q1_2025_Mine_Stats.xlsx", "Minestats3", "B32"),
    ("Barrick_Q2_2025_AISC.xlsx", "YTD Q2 2025", "Africa & Middle East", "AISC"):
        ("Barrick_Q2_2025_Mine_Stats.xlsx", "Minestats3", "B74"),
}


def run(root):
    d = os.path.join(root, "corpus_of_input_docs/downloaded/mine_stats")
    res = []  # (file, sheet, col, subject, family, state, detail)
    sheets = 0
    for f in sorted(glob.glob(os.path.join(d, "*AISC.xlsx"))):
        fn = os.path.basename(f)
        allc = load(f)
        for sh in visible(f):
            c = allc[sh]
            lab = {r: norm(c["A%d" % r]).lower() for r in range(1, 200) if c.get("A%d" % r) is not None}
            one = lambda *n: [r for r, l in lab.items() if l in n]
            rc, rt, ra = one("cost of sales applicable to gold production"), one("total cash costs"), \
                one("all-in sustaining costs")
            if not rc:
                continue
            sheets += 1
            rc, rt, ra = rc[0], rt[0], ra[0]
            ro = [r for r, l in lab.items() if l.startswith("ounces sold")][0]
            rates = {"COS": one("cost of sales per ounce", "cos/oz")[0],
                     "TCC": one("total cash costs per ounce", "tcc/oz")[0],
                     "AISC": one("all-in sustaining costs per ounce", "aisc/oz")[0]}
            b1 = [r for r in sorted(lab) if rc <= r < rt]
            b2 = [r for r in sorted(lab) if rt < r < ra]
            nci = [r for r in b1 + b2 if lab[r] == "non-controlling interests"]
            best, hr = -1, None
            for r in range(1, rc):
                n = sum(1 for i in range(2, 60) if subject(c.get(colname(i) + str(r), "")))
                if n > best:
                    best, hr = n, r
            for i in range(2, 60):
                col = colname(i)
                h = c.get(f"{col}{hr}")
                if h is None or norm(h).lower() == "footnote":
                    continue
                subj = subject(h)
                w = (fn, sh, col, subj or norm(h))
                if subj is None:
                    res.append(w + ("column", "v", "unregistered header"))
                    continue
                used = b1 + [rt] + b2 + [ra, ro] + list(rates.values())
                vs = {r: val(c, f"{col}{r}") for r in used}
                if all(v[0] == "place" or (v[0] == "num" and v[1] == 0) for v in vs.values()):
                    for fam in ("TCC", "AISC", "COS/oz", "TCC/oz", "AISC/oz"):
                        res.append(w + (fam, "np", "all-zero column"))
                    continue
                isint = lambda r: vs[r][0] == "num" and vs[r][1] == int(vs[r][1])
                lineval = lambda r: 0 if vs[r][0] == "place" else int(vs[r][1])
                # footing
                for fam, tot, lines in (("TCC", rt, b1), ("AISC", ra, [rt] + b2)):
                    if vs[tot][0] != "num":
                        res.append(w + (fam, "np", "total not published")); continue
                    bad = [r for r in lines if vs[r][0] in ("blank", "text")]
                    if bad:
                        res.append(w + (fam, "v", "line not numeric")); continue
                    if not all(isint(r) or vs[r][0] == "place" for r in lines + [tot]):
                        res.append(w + (fam, "v", "unrounded stored value")); continue
                    s = sum(lineval(r) for r in lines)
                    p = int(vs[tot][1])
                    alt = abs(p - s) * 2 <= 1 + (len([r for r in lines if vs[r][0] == "num"]) if fam == "TCC"
                                                 else len([r for r in b1 + b2 if vs[r][0] == "num"]))
                    res.append(w + (fam, "i" if s == p else "iii", (p, s, alt)))
                # per ounce
                q = vs[ro]
                for m, rr in rates.items():
                    fam = m + "/oz"
                    rv = vs[rr]
                    if rv[0] != "num":
                        res.append(w + (fam, "np", "rate not published")); continue
                    if q[0] != "num":
                        res.append(w + (fam, "v", "ounces not numeric")); continue
                    if rv[1] == 0 and q[1] == 0:
                        res.append(w + (fam, "np", "zero rate and ounces")); continue
                    if rv[1] == 0 or q[1] == 0:
                        res.append(w + (fam, "v", "one of rate and ounces is zero")); continue
                    if m == "COS":
                        nrows, numr = [rc], rc
                    elif m == "TCC":
                        nrows, numr = b1, rt
                    else:
                        nrows, numr = b1 + b2, ra
                    cells = [rr, ro, numr] + nrows
                    if any(vs[r][0] in ("blank", "text") for r in cells):
                        res.append(w + (fam, "v", "numerator line not numeric")); continue
                    if not all(isint(r) or vs[r][0] == "place" for r in cells):
                        res.append(w + (fam, "v", "unrounded stored value")); continue
                    if m == "COS" and any(vs[r][0] == "num" and vs[r][1] != 0 for r in nci):
                        res.append(w + (fam, "v", "attributable share needed")); continue
                    k = sum(1 for r in nrows if vs[r][0] == "num")
                    n, qq, r_ = int(vs[numr][1]), int(q[1]), int(rv[1])
                    corners = [F(1000) * (F(n) + a * F(k, 2)) / (F(qq) + b * F(1, 2)) for a in (-1, 1) for b in (-1, 1)]
                    lo, hi = min(corners), max(corners)
                    ok = not (hi < r_ - F(1, 2) or lo > r_ + F(1, 2))
                    c1 = [F(1000) * (F(n) + a * F(1, 2)) / (F(qq) + b * F(1, 2)) for a in (-1, 1) for b in (-1, 1)]
                    alt = not (max(c1) < r_ - F(1, 2) or min(c1) > r_ + F(1, 2))
                    if ok:
                        res.append(w + (fam, "i", ("alt", alt)))
                        continue
                    key = (fn, sh, subj, m)
                    if key in RECON:
                        mf, ms, mc = RECON[key]
                        comp = val(load(os.path.join(d, mf))[ms], mc)
                        if comp[0] == "num":
                            cv = F(int(comp[1]))
                            need_lo = F(r_ * 2 - 1, 2) * F(qq * 2 - 1, 2) / 1000
                            need_hi = F(r_ * 2 + 1, 2) * F(qq * 2 + 1, 2) / 1000
                            have_lo = n - F(k, 2) + cv - F(1, 2)
                            have_hi = n + F(k, 2) + cv + F(1, 2)
                            if not (have_hi < need_lo or have_lo > need_hi):
                                res.append(w + (fam, "ii", f"{mf}:{ms}!{mc}={comp[1]}")); continue
                    res.append(w + (fam, "iv", (n, qq, r_, float(lo), float(hi))))
    return sheets, res


if __name__ == "__main__":
    sheets, res = run(sys.argv[1])
    from collections import Counter
    print("AISC tables (visible):", sheets, "instances:", len(res))
    print(Counter((x[4] if "/" not in x[4] else "per-oz", x[5]) for x in res))
    print(Counter((x[4], x[5]) for x in res))
    alt_f = sum(1 for x in res if x[4] in ("TCC", "AISC") and x[5] in ("i", "iii") and x[6][2])
    alt_p = sum(1 for x in res if "/" in x[4] and x[5] == "i" and x[6][1])
    print("alt construction: footing consistent", alt_f, "per-oz consistent (of declared-consistent)", alt_p)
    for x in res:
        if x[5] in ("ii", "iii", "iv", "v"):
            print(" ", x[5].ljust(3), x[0][8:15], repr(x[1]).ljust(14), x[2], x[3].ljust(22), x[4].ljust(7), x[6])
