"""Independent extraction-fidelity reader (oracle for GATE-BULL-01 part 1; written separately from fmine-io).

Spec (shared with fmine-public's `numeric_cells_digest`, DESIGN):
- visible sheets only (state hidden/veryHidden are skipped), in workbook.xml order
- a cell counts when it has a value: <v> text, or inline <is><t> text
- t="e" -> formula error; t="s"/"str"/"inlineStr" -> string; else numeric if it parses as a finite float
- strings: trimmed text in {-, –, —, N/A, n/a, NA, na} -> placeholder; Excel error tokens -> formula error; else text
- digest = sha256 over "sheet\tREF\tvalue\n" for numeric cells, sheets in order, cells by (row, col);
  value printed as the shortest round-trip decimal in positional notation (integers without ".0")
usage: python3 -I fidelity.py <file.xlsx>...
"""
import hashlib, html, math, re, sys, zipfile
from decimal import Decimal

PLACE = {"-", "–", "—", "N/A", "n/a", "NA", "na"}
ERRS = {"#REF!", "#DIV/0!", "#VALUE!", "#NAME?", "#N/A", "#NUM!"}


def fmt(v):
    if v == 0 and math.copysign(1.0, v) < 0:
        return "-0"
    if v == int(v):
        return str(int(v))
    return format(Decimal(repr(v)), "f")


def ref_key(ref):
    m = re.fullmatch(r"\$?([A-Z]+)\$?(\d+)", ref)
    col = 0
    for ch in m.group(1):
        col = col * 26 + (ord(ch) - 64)
    return int(m.group(2)), col


def read(path):
    z = zipfile.ZipFile(path)
    names = set(z.namelist())
    shared = []
    if "xl/sharedStrings.xml" in names:
        x = z.read("xl/sharedStrings.xml").decode("utf-8")
        for si in re.findall(r"<si(?:\s[^>]*)?>(.*?)</si>", x, re.S):
            shared.append("".join(html.unescape(t) for t in re.findall(r"<t(?:\s[^>]*)?>(.*?)</t>", si, re.S)))
    wb = z.read("xl/workbook.xml").decode("utf-8")
    rels = z.read("xl/_rels/workbook.xml.rels").decode("utf-8")
    target = {}
    for tag in re.findall(r"<Relationship\b[^>]*>", rels):
        i = re.search(r'\bId="([^"]+)"', tag)
        t = re.search(r'\bTarget="([^"]+)"', tag)
        target[i.group(1)] = t.group(1)
    out = {"visible": 0, "hidden": 0, "numeric": 0, "placeholder": 0, "error": 0, "text": 0}
    h = hashlib.sha256()
    for tag in re.findall(r"<sheet\b[^>]*>", wb):
        name = html.unescape(re.search(r'\bname="([^"]*)"', tag).group(1))
        state = re.search(r'\bstate="([^"]*)"', tag)
        if state and state.group(1) in ("hidden", "veryHidden"):
            out["hidden"] += 1
            continue
        rid = re.search(r'\br:id="([^"]+)"', tag).group(1)
        key = target[rid].lstrip("/")
        key = key if key.startswith("xl/") else "xl/" + key
        if key not in names:
            continue
        out["visible"] += 1
        x = z.read(key).decode("utf-8")
        cells = []
        for m in re.finditer(r"<c\b([^>]*?)(/?)>", x):
            attrs, selfclose = m.group(1), m.group(2)
            if selfclose:
                continue
            end = x.index("</c>", m.end())
            body = x[m.end():end]
            r = re.search(r'\br="([^"]+)"', attrs)
            if not r:
                continue
            t = re.search(r'\bt="([^"]+)"', attrs)
            t = t.group(1) if t else ""
            v = re.search(r"<v(?:\s[^>]*)?>(.*?)</v>", body, re.S)
            if v is None:
                v = re.search(r"<t(?:\s[^>]*)?>(.*?)</t>", body, re.S)
            raw = v.group(1) if v else ""
            if t == "e":
                kind, val = "error", None
            elif t == "s":
                kind, val = "string", shared[int(raw.strip())]
            elif t in ("str", "inlineStr"):
                kind, val = "string", html.unescape(raw)
            else:
                s = raw.strip()
                if not s:
                    continue
                try:
                    f = float(s)
                    if not math.isfinite(f):
                        continue
                    kind, val = "number", f
                except ValueError:
                    kind, val = "string", html.unescape(s)
            if kind == "string":
                st = val.strip()
                kind = "placeholder" if st in PLACE else ("error" if st in ERRS else "text")
            cells.append((ref_key(r.group(1)), r.group(1), kind, val))
        cells.sort(key=lambda c: c[0])
        for _, ref, kind, val in cells:
            out["numeric" if kind == "number" else kind] += 1
            if kind == "number":
                h.update(f"{name}\t{ref}\t{fmt(val)}\n".encode("utf-8"))
    out["digest"] = h.hexdigest()
    return out


if __name__ == "__main__":
    for p in sys.argv[1:]:
        o = read(p)
        print(p.rsplit("/", 1)[-1], o["visible"], o["hidden"], o["numeric"], o["placeholder"], o["error"],
              o["text"], o["digest"])
