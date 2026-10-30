"""Independent census of the AISC identity ledger (reference read for the Rust implementation).

Semantics (DESIGN, mirrored in crates/fmine-public/src/identity.rs):
- header row: first row 1..10 holding a known subject name in columns B..; 'Footnote' ignored
- footing (component-sum construction): TCC = sum of lines from COS row up to (not incl.) TCC row;
  AISC = TCC + lines strictly between TCC and AISC rows; exact comparison
- per-ounce: 1000*N/Q on intervals; N half-width 0.5 per numeric line (COS: 1 line; TCC: block 1; AISC: both
  blocks); placeholders are exact nil; Q +-0.5; printed rate +-0.5
- COS/oz refused when any non-controlling-interest line is non-zero (attributable share needed)
- total cell placeholder/blank -> not published (excluded by name); blank line cell -> refused
usage: python3 -I census.py <workspace_root>
"""
import sys, glob, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xl import load

SUBJECTS = ["Nevada Gold Mines", "Carlin", "Cortez", "Turquoise Ridge", "Phoenix", "Long Canyon", "Hemlo",
            "Pueblo Viejo", "Veladero", "Porgera", "Loulo-Gounkoto", "Kibali", "North Mara", "Tongon",
            "Bulyanhulu", "North America", "Latin America & Asia Pacific", "Africa & Middle East"]
PLACE = {"-", "–", "—", "N/A", "n/a", "NA", "na"}


def norm(s):
    return " ".join(str(s).replace("&amp;", "&").split()).strip()


def subject(h):
    h = norm(h)
    for n in sorted(SUBJECTS, key=len, reverse=True):
        if h == n or (h.startswith(n) and len(h) == len(n) + 1 and h[-1].islower()):
            return n
    return None


def colname(i):
    s = ""
    while i:
        i, r = divmod(i - 1, 26)
        s = chr(65 + r) + s
    return s


def num(v):
    """('n', float) | ('p', None) placeholder | ('t', text) | None blank"""
    if v is None:
        return None
    s = str(v).strip()
    if s in PLACE:
        return ('p', None)
    try:
        return ('n', float(s))
    except ValueError:
        return ('t', s)


def run(root):
    files = sorted(glob.glob(os.path.join(root, "corpus_of_input_docs/downloaded/mine_stats/*AISC.xlsx")))
    out = {"footing": {}, "peroz": {}, "notpub": 0, "refused": [], "disc": [], "undec": [], "cols": 0,
           "unknown_headers": []}
    for f in files:
        fn = os.path.basename(f)
        for sh, c in load(f).items():
            hr = None
            for r in range(1, 11):
                if any(subject(c.get(colname(i) + str(r), "")) for i in range(2, 40)):
                    hr = r
                    break
            if hr is None:
                continue
            lab = {}
            for r in range(1, 80):
                a = c.get("A%d" % r)
                if a is not None:
                    lab[r] = norm(a).lower()

            def find(*names):
                for r, l in lab.items():
                    if l in names:
                        return r
                return None
            rc = find("cost of sales applicable to gold production")
            rt = find("total cash costs")
            ra = find("all-in sustaining costs")
            ro = next((r for r, l in lab.items() if l.startswith("ounces sold")), None)
            rates = {"COS": find("cost of sales per ounce", "cos/oz"),
                     "TCC": find("total cash costs per ounce", "tcc/oz"),
                     "AISC": find("all-in sustaining costs per ounce", "aisc/oz")}
            b1 = [r for r in sorted(lab) if rc <= r < rt]
            b2 = [r for r in sorted(lab) if rt < r < ra]
            nci = [r for r in b1 + b2 if lab[r] == "non-controlling interests"]
            for i in range(2, 40):
                h = c.get(colname(i) + str(hr))
                if h is None or norm(h) == "Footnote":
                    continue
                subj = subject(h)
                col = colname(i)
                if subj is None:
                    out["unknown_headers"].append((fn, sh, col, norm(h)))
                    continue
                out["cols"] += 1
                where = (fn, sh, col, subj)
                tcc, aisc = num(c.get(f"{col}{rt}")), num(c.get(f"{col}{ra}"))
                if tcc is None or aisc is None or tcc[0] != 'n' or aisc[0] != 'n':
                    out["notpub"] += 1
                    continue
                cells = {r: num(c.get(f"{col}{r}")) for r in b1 + b2}
                bad = [r for r, v in cells.items() if v is None or v[0] == 't']
                if bad:
                    out["refused"].append(where + ("blank or text line at row %d" % bad[0],))
                    continue
                val = {r: (v[1] if v[0] == 'n' else 0.0) for r, v in cells.items()}
                nn = {r: (v[0] == 'n') for r, v in cells.items()}
                s1 = sum(val[r] for r in b1)
                s2 = tcc[1] + sum(val[r] for r in b2)
                for name, lhs, rhs in (("TCC", tcc[1], s1), ("AISC", aisc[1], s2)):
                    ok = abs(lhs - rhs) < 1e-9
                    out["footing"][ok] = out["footing"].get(ok, 0) + 1
                    if not ok:
                        out["disc"].append(where + (name, lhs, rhs))
                oz = num(c.get(f"{col}{ro}"))
                for m, rr in rates.items():
                    if rr is None:
                        continue
                    rate = num(c.get(f"{col}{rr}"))
                    if rate is None or rate[0] != 'n' or oz is None or oz[0] != 'n' or oz[1] <= 0.5:
                        k = "notpub"
                        out["peroz"][k] = out["peroz"].get(k, 0) + 1
                        continue
                    if m == "COS" and any(val[r] != 0 for r in nci):
                        k = "refused"
                        out["peroz"][k] = out["peroz"].get(k, 0) + 1
                        continue
                    if m == "COS":
                        v = num(c.get(f"{col}{rc}"))
                        n_val, n_lines = v[1], 1
                    elif m == "TCC":
                        n_val, n_lines = tcc[1], sum(1 for r in b1 if nn[r])
                    else:
                        n_val, n_lines = aisc[1], sum(1 for r in b1 + b2 if nn[r])
                    w = 0.5 * n_lines
                    nlo, nhi = n_val - w, n_val + w
                    qlo, qhi = oz[1] - 0.5, oz[1] + 0.5
                    corners = [1000 * a / b for a in (nlo, nhi) for b in (qlo, qhi)]
                    lo, hi = min(corners), max(corners)
                    ok = not (hi < rate[1] - 0.5 or lo > rate[1] + 0.5)
                    k = "consistent" if ok else "outside"
                    out["peroz"][k] = out["peroz"].get(k, 0) + 1
                    if not ok:
                        out["undec"].append(where + (m, n_val, oz[1], rate[1], round(lo, 1), round(hi, 1)))
    return out


if __name__ == "__main__":
    o = run(sys.argv[1])
    print("columns", o["cols"], "not published", o["notpub"], "refused", len(o["refused"]))
    print("unknown headers", o["unknown_headers"])
    print("footing", o["footing"])
    for d in o["disc"]:
        print("  DISC", d)
    print("per-oz", o["peroz"])
    for u in o["undec"]:
        print("  OUT", u)
    for r in o["refused"]:
        print("  REF", r)
