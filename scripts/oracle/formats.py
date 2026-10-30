"""Independent number-format census of xlsx workbooks (oracle for bd-jg43.1; written before the Rust styles reader).

Spec:
- visible sheets only (state hidden/veryHidden skipped), as fidelity.py (E820);
- a cell is numeric exactly as in fidelity.py: not t="e", "s", "str" or "inlineStr", and its stripped <v> text
  (or inline <t> text) parses as a finite float;
- its format: the cell's s attribute (absent -> 0) indexes <cellXfs>; an index past the end is
  "bad style index <s>". The xf's numFmtId (absent -> 0) resolves to the <numFmts> code with that id if there is
  one, else the ECMA-376 18.8.30 built-in code, else "unknown numFmtId <id>". With no styles part every cell is
  "General";
- the styles part is the workbook relationship whose Type ends in /styles, falling back to xl/styles.xml;
- non-integer is decided on the stored text as an exact decimal (exponent accepted), never on a float.

Output: one JSON line per file, {"file","sha256","formats":{code:[numeric,non_integer]}}, then
{"files","formats","numeric_total","non_integer_total"} with formats summed over files.
usage: python3 -I formats.py <file.xlsx>...
"""
import hashlib, json, math, posixpath, sys, zipfile
from decimal import Decimal, InvalidOperation
from xml.etree import ElementTree as ET

M = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
R = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"
P = "{http://schemas.openxmlformats.org/package/2006/relationships}"

# ECMA-376 Part 1, 18.8.30 (numFmt): the built-in formats the standard defines.
BUILTIN = {
    0: "General", 1: "0", 2: "0.00", 3: "#,##0", 4: "#,##0.00", 9: "0%", 10: "0.00%", 11: "0.00E+00",
    12: "# ?/?", 13: "# ??/??", 14: "mm-dd-yy", 15: "d-mmm-yy", 16: "d-mmm", 17: "mmm-yy", 18: "h:mm AM/PM",
    19: "h:mm:ss AM/PM", 20: "h:mm", 21: "h:mm:ss", 22: "m/d/yy h:mm", 37: "#,##0 ;(#,##0)",
    38: "#,##0 ;[Red](#,##0)", 39: "#,##0.00;(#,##0.00)", 40: "#,##0.00;[Red](#,##0.00)", 45: "mm:ss",
    46: "[h]:mm:ss", 47: "mmss.0", 48: "##0.0E+0", 49: "@",
}


def part(base, target):
    if target.startswith("/"):
        return target.lstrip("/")
    return posixpath.normpath(posixpath.join(base, target))


def styles_of(z, names, rels):
    key = None
    for r in rels:
        if r.get("Type", "").endswith("/styles"):
            key = part("xl", r.get("Target"))
    if key is None or key not in names:
        key = "xl/styles.xml" if "xl/styles.xml" in names else None
    if key is None:
        return None
    st = ET.fromstring(z.read(key))
    custom = {}
    nf = st.find(M + "numFmts")
    if nf is not None:
        for f in nf.findall(M + "numFmt"):
            custom[int(f.get("numFmtId"))] = f.get("formatCode")
    xfs = []
    cx = st.find(M + "cellXfs")
    if cx is not None:
        xfs = [int(x.get("numFmtId", "0")) for x in cx.findall(M + "xf")]
    return custom, xfs


def code_of(styles, s):
    if styles is None:
        return "General"
    custom, xfs = styles
    if s >= len(xfs):
        return "bad style index %d" % s
    fid = xfs[s]
    if fid in custom:
        return custom[fid]
    return BUILTIN.get(fid, "unknown numFmtId %d" % fid)


def non_integer(text):
    try:
        d = Decimal(text)
    except InvalidOperation:
        return False
    return d != d.to_integral_value()


def read(path):
    data = open(path, "rb").read()
    z = zipfile.ZipFile(path)
    names = set(z.namelist())
    wb = ET.fromstring(z.read("xl/workbook.xml"))
    rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels")).findall(P + "Relationship")
    target = {r.get("Id"): r.get("Target") for r in rels}
    styles = styles_of(z, names, rels)
    counts = {}
    for sh in wb.find(M + "sheets").findall(M + "sheet"):
        if sh.get("state", "visible") in ("hidden", "veryHidden"):
            continue
        key = part("xl", target[sh.get(R + "id")])
        if key not in names:
            continue
        for c in ET.fromstring(z.read(key)).iter(M + "c"):
            t = c.get("t", "")
            if t in ("e", "s", "str", "inlineStr"):
                continue
            v = c.find(M + "v")
            if v is None:
                v = c.find(M + "is/" + M + "t")
            text = (v.text or "").strip() if v is not None else ""
            if not text:
                continue
            try:
                if not math.isfinite(float(text)):
                    continue
            except ValueError:
                continue
            code = code_of(styles, int(c.get("s", "0")))
            n = counts.setdefault(code, [0, 0])
            n[0] += 1
            n[1] += non_integer(text)
    return {"file": posixpath.basename(path), "sha256": hashlib.sha256(data).hexdigest(),
            "formats": dict(sorted(counts.items()))}


if __name__ == "__main__":
    total = {}
    files = 0
    for p in sys.argv[1:]:
        o = read(p)
        files += 1
        print(json.dumps(o, ensure_ascii=False, separators=(",", ":")))
        for code, (n, ni) in o["formats"].items():
            t = total.setdefault(code, [0, 0])
            t[0] += n
            t[1] += ni
    print(json.dumps({"files": files, "formats": dict(sorted(total.items())),
                      "numeric_total": sum(n for n, _ in total.values()),
                      "non_integer_total": sum(ni for _, ni in total.values())},
                     ensure_ascii=False, separators=(",", ":")))
