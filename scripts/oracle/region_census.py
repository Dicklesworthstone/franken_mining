"""Region census: each region's attributable cost of sales per ounce checked against its member sites, weighted by
the members' attributable ounces sold. Scope: every instance share_census.py leaves in state (v) "regional
numerator needs member weighting" (its `regions_v`).

Independent reader, written 2026-10-30 without opening any Rust under crates/, crates/fmine-public/tests/laap_2023.rs,
planning/mor/MOR_CASE.md or any other agent's scratch files. It imports census2.py, share_census.py and
hidden_census.py only to enumerate the in-scope instances, and formats.py only for its number-format interpreter
(styles_of, code_of, displayed). Every row, header, member and operand it uses is located here, by text.

Rule (applied as stated, not tuned):
1. Scope: the instances share_census.run(root) counts as regions_v, on visible sheets; with --hidden, on the hidden
   and very hidden AISC sheets, by swapping census2.visible for hidden_census.hidden exactly as hidden_census.py
   does (Mine Statistics files keep their visible sheets).
2. Values are read as displayed (formats.py --displayed): a cell is a whole displayed number (a shown text of
   digits with an optional minus, no "." and no "%"), a zero-section token (a numeric 0 shown as a dash), or a
   text placeholder (census2.PLACE: "-", "–", "—", "N/A", "n/a", "NA", "na", after whitespace normalisation).
   Anything else (blank, other text, decimals or a percentage displayed, a format formats.py refuses, a numeric
   string cell) is none of the three.
3. Region map, keyed by the AISC file's registry release quarter ([[source]].quarter in
   registries/sources/barrick.toml), never by the sheet's period (W1 §13 rule 2):
   releases through 2025Q3: North America = {Nevada Gold Mines, Hemlo}; Latin America & Asia Pacific =
   {Pueblo Viejo, Veladero, Porgera}; Africa & Middle East = {Loulo-Gounkoto, Kibali, North Mara, Tongon,
   Bulyanhulu}. Releases from 2025Q4: North America = {Nevada Gold Mines, Pueblo Viejo, Hemlo}; South America &
   Asia Pacific = {Veladero, Porgera}; Africa & Middle East as above. Carlin, Cortez, Turquoise Ridge, Phoenix
   and Long Canyon are Nevada Gold Mines' own sites and never members.
4. Members on the sheet: a registered member whose header (census2.subject: the subject name, optionally followed
   by one lowercase footnote letter) is on the table's header row. A member with no column there is `absent`.
5. Member operands: its cells on the "Cost of sales per ounce" row (rate) and the "Ounces sold" row (attributable
   ounces, 000s oz). Both whole displayed numbers with ounces not zero -> `used`; rate and ounces both zero
   (displayed 0 or a zero-section token) or both text placeholders -> `excluded` (no_ounces); anything else ->
   the instance is refused, reason member_operand, at the member's cell. No member used -> refused, no_members.
6. Weight footing (component sum, E815): the used members' printed ounces must sum exactly to the region's
   printed ounces, else refused, region_ounces_do_not_foot.
7. Check: each printed integer x stands for [x - 1/2, x + 1/2]. lo = min over the 2^n vertices w of the used
   members' ounce box of sum((r_i - 1/2) w_i) / sum(w_i); hi = max over the vertices of sum((r_i + 1/2) w_i) /
   sum(w_i); exact Fractions. With R the region's printed rate: refuted iff hi < R - 1/2 or lo > R + 1/2, else
   not_refuted.

Layout, located by text on each table's own sheet: the cost-of-sales row (column A "Cost of sales applicable to
gold production"), the ounces row (column A starting "Ounces sold"), the rate row (column A "Cost of sales per
ounce" or "COS/oz"), each required to be unique; the header row is the row above the cost-of-sales row holding the
most registered-subject headers (first on ties), counted over every column but A. The instance's own header there
must name its region.

Judgement calls (the rule is silent or ambiguous here):
- J1 A zero-section token is not a whole displayed number: rule 2 lists the three kinds separately, so a dash
  rate with non-zero ounces is "anything else" (refused), not a rate of 0.
- J2 Mixed zero kinds: a displayed 0 and a token count as zero together; a zero beside a text placeholder is
  "anything else" (rule 5 says both zero or both placeholders).
- J3 The member_operand cell is the first of (rate, ounces) that is not a whole displayed number; when both are
  whole (non-zero rate beside zero ounces, or J4), the ounces cell. `at` is a bare A1 reference on the
  instance's sheet; no_members and region_ounces_do_not_foot name no cell, so their `at` is null.
- J4 A used member's ounces must be positive: rule 7's argument (the mean increases in each r_i; extremes at box
  vertices) needs every weight positive, so negative printed ounces are refused as member_operand at the ounces
  cell. Never fires on the 2026-10-30 corpus.
- J5 Region operands: the region's own ounces (rule 6) and rate (rule 7) must be whole displayed numbers; if
  not, the instance is refused with reason region_operand at that cell (a code the rule does not list; census2
  already requires both to be printed integers on stored values, so this only fires if a format hides that).
  Never fires on the 2026-10-30 corpus.
- J6 Outside the rule, the script stops (SystemExit) rather than invent a verdict: a region name not in its
  release's map (e.g. "Latin America & Asia Pacific" in a 2025Q4+ release), a member header in two columns, a
  non-unique layout row, or this reader's displayed text disagreeing with formats.read_displayed on a visible sheet.
- J7 Lists are in the region map's order as written in rule 3, not sheet column order; `excluded` holds names
  only (no_ounces is the only exclusion reason).
- J8 formats.read_displayed skips hidden sheets, so this script reads each table's sheet itself with formats.py's
  own numeric-cell test and interpreter, whatever the sheet's state; on every visible sheet of each file it reads
  (in both scopes) it must agree exactly with formats.read_displayed (J6).

Output: one JSON line per in-scope instance
  {"file","sheet","cell","region","members","absent","excluded","verdict","reason","at"}
(cell: the region's rate cell; members: used member names), then {"regions","refuted","not_refuted","refused"}.
No cell value, lo, hi or margin is printed (RISK-31) unless --values is given, which adds a "values" object.
usage: python3 -I scripts/oracle/region_census.py <workspace_root> [--hidden] [--values]
"""
import html
import itertools
import json
import math
import os
import re
import sys
import tomllib
import zipfile
from fractions import Fraction as F
from xml.etree import ElementTree as ET

sys.dont_write_bytecode = True  # leave no __pycache__ entries behind in the repository
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xl import load  # noqa: E402
import census2  # noqa: E402
import formats  # noqa: E402
import hidden_census  # noqa: E402
import share_census  # noqa: E402

HALF = F(1, 2)
NGM_OWN_SITES = {"Carlin", "Cortez", "Turquoise Ridge", "Phoenix", "Long Canyon"}
AFRICA = ["Loulo-Gounkoto", "Kibali", "North Mara", "Tongon", "Bulyanhulu"]
MAP_TO_2025Q3 = {
    "North America": ["Nevada Gold Mines", "Hemlo"],
    "Latin America & Asia Pacific": ["Pueblo Viejo", "Veladero", "Porgera"],
    "Africa & Middle East": AFRICA,
}
MAP_FROM_2025Q4 = {
    "North America": ["Nevada Gold Mines", "Pueblo Viejo", "Hemlo"],
    "South America & Asia Pacific": ["Veladero", "Porgera"],
    "Africa & Middle East": AFRICA,
}
for _m in (MAP_TO_2025Q3, MAP_FROM_2025Q4):
    assert not NGM_OWN_SITES & {s for ms in _m.values() for s in ms}


def region_map(quarter):
    m = re.fullmatch(r"([0-9]{4})Q([1-4])", quarter)
    if not m:
        raise SystemExit("unreadable registry quarter %r" % quarter)
    return MAP_TO_2025Q3 if (int(m.group(1)), int(m.group(2))) <= (2025, 3) else MAP_FROM_2025Q4


def release_quarters(root):
    with open(os.path.join(root, "registries/sources/barrick.toml"), "rb") as fh:
        reg = tomllib.load(fh)
    return os.path.join(root, reg["corpus_dir"]), \
        {s["file"]: s["quarter"] for s in reg["source"] if s["kind"] == "aisc_reconciliation"}


def rc_split(ref):
    m = re.fullmatch(r"([A-Z]+)([0-9]+)", ref)
    return (m.group(1), int(m.group(2))) if m else (None, None)


def sheet_displayed(path, sheet):
    """{cell: (kind, text)} for the numeric cells of one sheet, whatever its state, by formats.py's own numeric-cell
    test and interpreter (formats.read_displayed reads visible sheets only)."""
    z = zipfile.ZipFile(path)
    names = set(z.namelist())
    wb = ET.fromstring(z.read("xl/workbook.xml"))
    rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels")).findall(formats.P + "Relationship")
    target = {r.get("Id"): r.get("Target") for r in rels}
    styles = formats.styles_of(z, names, rels)
    hits = [s for s in wb.find(formats.M + "sheets").findall(formats.M + "sheet")
            if s.get("name") == html.unescape(sheet)]
    if len(hits) != 1:
        raise SystemExit("%s: sheet %r found %d times" % (os.path.basename(path), sheet, len(hits)))
    out = {}
    for c in ET.fromstring(z.read(formats.part("xl", target[hits[0].get(formats.R + "id")]))).iter(formats.M + "c"):
        if c.get("t", "") in ("e", "s", "str", "inlineStr"):
            continue
        v = c.find(formats.M + "v")
        if v is None:
            v = c.find(formats.M + "is/" + formats.M + "t")
        text = (v.text or "").strip() if v is not None else ""
        if not text:
            continue
        try:
            if not math.isfinite(float(text)):
                continue
        except ValueError:
            continue
        if formats.row_col(c.get("r")) is None:
            continue
        out[c.get("r")] = formats.displayed(text, formats.code_of(styles, int(c.get("s", "0"))))
    return out


def operand(cells, shown, ref):
    """(kind, int or None): kind is whole, token, place, or other (rule 2)."""
    if ref in shown:
        kind, text = shown[ref]
        if kind == "token":
            return ("token", 0)
        if kind == "shown" and re.fullmatch(r"-?[0-9]+", text):
            return ("whole", int(text))
        return ("other", None)
    v = cells.get(ref)
    if v is not None and census2.norm(v) in census2.PLACE:
        return ("place", None)
    return ("other", None)


def layout(cells):
    """(cost-of-sales row, ounces row, rate row, header row, {header row column: subject})."""
    lab = {}
    for ref, v in cells.items():
        col, row = rc_split(ref)
        if col == "A":
            lab[row] = census2.norm(v).lower()
    rows = lambda pred: sorted(r for r, t in lab.items() if pred(t))  # noqa: E731
    rc = rows(lambda t: t == "cost of sales applicable to gold production")
    ro = rows(lambda t: t.startswith("ounces sold"))
    rr = rows(lambda t: t in ("cost of sales per ounce", "cos/oz"))
    if len(rc) != 1 or len(ro) != 1 or len(rr) != 1:
        raise SystemExit("ambiguous AISC layout: cos %r ounces %r rate %r" % (rc, ro, rr))
    heads = {}
    for ref, v in cells.items():
        col, row = rc_split(ref)
        if col not in (None, "A") and row < rc[0]:
            subj = census2.subject(v)
            if subj is not None:
                heads.setdefault(row, {})[col] = subj
    if not heads:
        raise SystemExit("no header row above the cost-of-sales row")
    hr = min(heads, key=lambda r: (-len(heads[r]), r))
    return rc[0], ro[0], rr[0], hr, heads[hr]


def is_zero(o):
    return o[0] == "token" or (o[0] == "whole" and o[1] == 0)


def evaluate(inst):
    """Rules 5-7 on one gathered instance; returns (verdict, reason, at, used, excluded, lo, hi)."""
    used, excluded = [], []
    for name, (rref, rate), (oref, oz) in inst["present"]:
        if rate[0] == "whole" and oz[0] == "whole" and oz[1] != 0:
            if oz[1] < 0:  # J4
                return "refused", "member_operand", oref, used, excluded, None, None
            used.append((name, rate[1], oz[1]))
        elif (is_zero(rate) and is_zero(oz)) or (rate[0] == "place" and oz[0] == "place"):
            excluded.append(name)
        else:  # J3
            at = rref if rate[0] != "whole" else oref
            return "refused", "member_operand", at, used, excluded, None, None
    if not used:
        return "refused", "no_members", None, used, excluded, None, None
    qref, q = inst["region_oz"]
    if q[0] != "whole":  # J5
        return "refused", "region_operand", qref, used, excluded, None, None
    if sum(w for _, _, w in used) != q[1]:
        return "refused", "region_ounces_do_not_foot", None, used, excluded, None, None
    rref, r = inst["region_rate"]
    if r[0] != "whole":  # J5
        return "refused", "region_operand", rref, used, excluded, None, None
    lo = hi = None
    for signs in itertools.product((-1, 1), repeat=len(used)):
        ws = [w + s * HALF for (_, _, w), s in zip(used, signs)]
        den = sum(ws)
        a = sum((x - HALF) * wi for (_, x, _), wi in zip(used, ws)) / den
        b = sum((x + HALF) * wi for (_, x, _), wi in zip(used, ws)) / den
        lo = a if lo is None else min(lo, a)
        hi = b if hi is None else max(hi, b)
    refuted = hi < r[1] - HALF or lo > r[1] + HALF
    return ("refuted" if refuted else "not_refuted"), None, None, used, excluded, lo, hi


def scope(root, hidden):
    """share_census's regions_v instances, visible or (hidden_census's swap) hidden AISC sheets."""
    if hidden:
        census2.visible = hidden_census.hidden
    try:
        rows = share_census.run(root)
    finally:
        census2.visible = hidden_census.VISIBLE
    return [rec for kind, rec in rows if kind == "regions_v"]


def gather(root, hidden=False):
    corpus, quarter = release_quarters(root)
    books, shown_by, checked = {}, {}, set()
    out = []
    for rec in scope(root, hidden):
        fn, sh, cell, region = rec["file"], rec["sheet"], rec["cell"], rec["subject"]
        path = os.path.join(corpus, fn)
        if fn not in books:
            books[fn] = load(path)
        cells = books[fn][sh]
        if (fn, sh) not in shown_by:
            shown_by[(fn, sh)] = sheet_displayed(path, sh)
        shown = shown_by[(fn, sh)]
        if fn not in checked:  # J6/J8: agree exactly with formats.read_displayed on every visible sheet
            into = {}
            formats.read_displayed(path, False, into)
            for s in {s for s, _ in into}:
                if sheet_displayed(path, s) != {ref: t for (s2, ref), t in into.items() if s2 == s}:
                    raise SystemExit("%s %s: displayed text disagrees with formats.py" % (fn, s))
            checked.add(fn)
        rc, ro, rr, hr, heads = layout(cells)
        col, row = rc_split(cell)
        if row != rr or heads.get(col) != region:
            raise SystemExit("%s %s!%s: not the %s rate cell under its header" % (fn, sh, cell, region))
        rmap = region_map(quarter[fn])
        if region not in rmap:
            raise SystemExit("%s %s!%s: region %r is not in the %s map" % (fn, sh, cell, region, quarter[fn]))
        present, absent = [], []
        for m in rmap[region]:
            cols = sorted((c for c, s in heads.items() if s == m), key=lambda c: (len(c), c))
            if len(cols) > 1:
                raise SystemExit("%s %s: member %r heads columns %r" % (fn, sh, m, cols))
            if not cols:
                absent.append(m)
                continue
            rref, oref = "%s%d" % (cols[0], rr), "%s%d" % (cols[0], ro)
            present.append((m, (rref, operand(cells, shown, rref)), (oref, operand(cells, shown, oref))))
        out.append({"file": fn, "sheet": sh, "cell": cell, "region": region, "absent": absent,
                    "present": present,
                    "region_rate": (cell, operand(cells, shown, cell)),
                    "region_oz": ("%s%d" % (col, ro), operand(cells, shown, "%s%d" % (col, ro)))})
    return out


def frac(x):
    return None if x is None else "%d/%d" % (x.numerator, x.denominator)


def main(argv):
    flags = {a for a in argv if a.startswith("--")}
    roots = [a for a in argv if not a.startswith("--")]
    if len(roots) != 1 or flags - {"--hidden", "--values"}:
        raise SystemExit("usage: region_census.py <workspace_root> [--hidden] [--values]")
    summary = {"regions": 0, "refuted": 0, "not_refuted": 0, "refused": 0}
    for inst in gather(roots[0], "--hidden" in flags):
        verdict, reason, at, used, excluded, lo, hi = evaluate(inst)
        rec = {"file": inst["file"], "sheet": inst["sheet"], "cell": inst["cell"], "region": inst["region"],
               "members": [n for n, _, _ in used], "absent": inst["absent"], "excluded": excluded,
               "verdict": verdict, "reason": reason, "at": at}
        if "--values" in flags:
            rec["values"] = {"rate": list(inst["region_rate"][1]), "ounces": list(inst["region_oz"][1]),
                             "members": {n: [list(r[1]), list(o[1])] for n, r, o in inst["present"]},
                             "lo": frac(lo), "hi": frac(hi)}
        print(json.dumps(rec, ensure_ascii=False))
        summary["regions"] += 1
        summary[verdict] += 1
    print(json.dumps(summary))


if __name__ == "__main__":
    main(sys.argv[1:])
